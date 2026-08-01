//! Audio preparation, WAV encoding, and whisper-cli transcription.

use crate::state::{
    AudioSignalStats, MAX_TRANSCRIPTION_AUDIO_GAIN, MIN_TRANSCRIPTION_AUDIO_PEAK,
    MIN_TRANSCRIPTION_AUDIO_RMS, TARGET_TRANSCRIPTION_AUDIO_PEAK, WHISPER_SAMPLE_RATE,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TRANSCRIPTION_TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Owns a unique temp directory for one transcription and removes it on drop.
struct TranscriptionTempDir {
    path: PathBuf,
}

impl TranscriptionTempDir {
    /// Creates a unique directory under the system temp dir for this transcription.
    ///
    /// Uses an atomic counter plus `create_dir` (fails if the path already exists) and
    /// retries a few times so concurrent transcriptions cannot collide.
    fn create() -> Result<Self, String> {
        const MAX_ATTEMPTS: u32 = 8;
        let pid = std::process::id();

        for _ in 0..MAX_ATTEMPTS {
            let tick = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let counter = TRANSCRIPTION_TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "dicktaint-transcribe-{pid}-{tick}-{counter}"
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "Failed to create transcription temp directory {}: {error}",
                        path.display()
                    ));
                }
            }
        }

        Err(
            "Failed to create a unique transcription temp directory after several attempts"
                .to_string(),
        )
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TranscriptionTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}


fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = target_rate as f32 / source_rate as f32;
    let out_len = ((samples.len() as f32) * ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f32 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = src_pos - idx as f32;

        let a = samples.get(idx).copied().unwrap_or(0.0);
        let b = samples.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }

    out
}

fn write_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create wav file {}: {e}", path.display()))?;
    for sample in samples {
        let clipped = sample.clamp(-1.0, 1.0);
        let pcm = (clipped * i16::MAX as f32) as i16;
        writer
            .write_sample(pcm)
            .map_err(|e| format!("Failed to write wav sample: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize wav file: {e}"))?;

    Ok(())
}

pub(crate) fn analyze_audio_signal(samples: &[f32], sample_rate: u32) -> AudioSignalStats {
    if samples.is_empty() || sample_rate == 0 {
        return AudioSignalStats {
            peak_abs: 0.0,
            rms: 0.0,
            duration_secs: 0.0,
        };
    }

    let mut peak_abs = 0.0_f32;
    let mut energy = 0.0_f64;
    for sample in samples {
        let abs = sample.abs();
        if abs > peak_abs {
            peak_abs = abs;
        }
        energy += f64::from(*sample) * f64::from(*sample);
    }

    AudioSignalStats {
        peak_abs,
        rms: (energy / samples.len() as f64).sqrt() as f32,
        duration_secs: samples.len() as f32 / sample_rate as f32,
    }
}

fn audio_signal_is_too_quiet(stats: AudioSignalStats) -> bool {
    stats.peak_abs < MIN_TRANSCRIPTION_AUDIO_PEAK && stats.rms < MIN_TRANSCRIPTION_AUDIO_RMS
}

fn normalize_audio_gain(samples: Vec<f32>, stats: AudioSignalStats) -> Vec<f32> {
    if stats.peak_abs <= 0.0 {
        return samples;
    }

    let gain = (TARGET_TRANSCRIPTION_AUDIO_PEAK / stats.peak_abs).min(MAX_TRANSCRIPTION_AUDIO_GAIN);
    if gain <= 1.0 {
        return samples;
    }

    samples
        .into_iter()
        .map(|sample| (sample * gain).clamp(-1.0, 1.0))
        .collect()
}

fn quiet_audio_error(stats: AudioSignalStats, input_device_name: &str) -> String {
    format!(
        "Captured audio from '{}' was too quiet to transcribe (peak {:.4}, rms {:.4}, {:.1}s). Check macOS Sound > Input, confirm the selected microphone, and retry.",
        input_device_name, stats.peak_abs, stats.rms, stats.duration_secs
    )
}

fn is_transcript_artifact_token(token: &str) -> bool {
    let normalized = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
    let upper = normalized.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "BLANK_AUDIO" | "NOISE" | "MUSIC" | "SILENCE"
    )
}

fn normalize_transcript_text(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|token| !is_transcript_artifact_token(token))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn transcribe_samples(
    model_path: PathBuf,
    whisper_cli_path: String,
    samples: Vec<f32>,
    sample_rate: u32,
    input_device_name: String,
) -> Result<String, String> {
    let prepared = if sample_rate == WHISPER_SAMPLE_RATE {
        samples
    } else {
        resample_linear(&samples, sample_rate, WHISPER_SAMPLE_RATE)
    };

    if prepared.is_empty() {
        return Err("No audio captured. Check microphone input and try again.".to_string());
    }

    let signal = analyze_audio_signal(&prepared, WHISPER_SAMPLE_RATE);
    if audio_signal_is_too_quiet(signal) {
        return Err(quiet_audio_error(signal, &input_device_name));
    }
    let prepared = normalize_audio_gain(prepared, signal);

    let temp_dir = TranscriptionTempDir::create()?;
    let wav_path = temp_dir.path().join("audio.wav");
    let out_prefix = temp_dir.path().join("transcript");
    let txt_path = out_prefix.with_extension("txt");

    write_wav(&wav_path, &prepared, WHISPER_SAMPLE_RATE)?;

    let output = Command::new(&whisper_cli_path)
    .arg("-m")
    .arg(&model_path)
    .arg("-f")
    .arg(&wav_path)
    .arg("-l")
    .arg("en")
    .arg("-otxt")
    .arg("-nt")
    .arg("-of")
    .arg(&out_prefix)
    .output()
    .map_err(|e| {
      format!(
        "Failed to execute whisper cli '{whisper_cli_path}': {e}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
      )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let mut detail = String::new();
        if !stderr.is_empty() {
            detail.push_str(&stderr);
        }
        if detail.is_empty() && !stdout.is_empty() {
            detail.push_str(&stdout);
        }
        if detail.is_empty() {
            detail.push_str("no error output");
        }
        return Err(format!("whisper-cli transcription failed: {detail}"));
    }

    let transcript = std::fs::read_to_string(&txt_path).map_err(|e| {
        format!(
            "whisper-cli ran but transcript file is missing at {}: {e}",
            txt_path.display()
        )
    })?;

    let cleaned = normalize_transcript_text(&transcript);
    if cleaned.is_empty() {
        return Err("No speech detected in the recorded audio.".to_string());
    }

    Ok(cleaned)
}



#[cfg(test)]
mod tests {
    use super::{
        analyze_audio_signal, audio_signal_is_too_quiet, normalize_audio_gain, quiet_audio_error,
        resample_linear,
    };

    #[test]
    fn resample_linear_returns_same_when_rate_matches() {
        let source = vec![0.0_f32, 0.5, -0.5, 1.0];
        let out = resample_linear(&source, 16_000, 16_000);
        assert_eq!(out, source);
    }

    #[test]
    fn resample_linear_produces_output_when_rate_changes() {
        let source = vec![0.0_f32, 1.0, 0.0, -1.0];
        let out = resample_linear(&source, 8_000, 16_000);
        assert!(out.len() > source.len());
        assert!(out.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn analyze_audio_signal_reports_peak_rms_and_duration() {
        let samples = vec![0.0_f32, 0.25, -0.5, 0.5];
        let stats = analyze_audio_signal(&samples, 8_000);
        assert!((stats.peak_abs - 0.5).abs() < 0.0001);
        assert!(stats.rms > 0.0);
        assert!(stats.duration_secs > 0.0);
    }

    #[test]
    fn quiet_audio_detection_flags_near_silent_capture() {
        let samples = vec![0.0002_f32; 16_000];
        let stats = analyze_audio_signal(&samples, 16_000);
        assert!(audio_signal_is_too_quiet(stats));
        assert!(quiet_audio_error(stats, "MacBook Pro Microphone").contains("too quiet"));
    }

    #[test]
    fn normalize_audio_gain_boosts_quiet_but_valid_audio() {
        let samples = vec![0.01_f32, -0.02, 0.03, -0.04];
        let stats = analyze_audio_signal(&samples, 16_000);
        assert!(!audio_signal_is_too_quiet(stats));
        let boosted = normalize_audio_gain(samples.clone(), stats);
        let boosted_peak = boosted
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        let original_peak = samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        assert!(boosted_peak > original_peak);
        assert!(boosted_peak <= 0.85);
    }
}
