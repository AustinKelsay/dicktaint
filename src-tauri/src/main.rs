use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::State;

const WHISPER_SAMPLE_RATE: u32 = 16_000;

#[derive(Clone)]
struct AppConfig {
    ollama_host: String,
    whisper_model_path: Option<String>,
    whisper_cli_path: Option<String>,
}

#[derive(Default)]
struct DictationState {
    active_recording: Mutex<Option<ActiveRecording>>,
}

struct ActiveRecording {
    stop_tx: mpsc::Sender<()>,
    thread_handle: thread::JoinHandle<()>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Serialize)]
struct GenerateOptions {
    temperature: f32,
}

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: GenerateOptions,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: Option<String>,
}

fn make_prompt(transcript: &str, instruction: &str) -> String {
    [
        instruction,
        "",
        "Raw transcript:",
        transcript,
        "",
        "Output only the cleaned dictation text.",
    ]
    .join("\n")
}

fn normalize_host(host: &str) -> String {
    host.trim_end_matches('/').to_string()
}

fn resolve_whisper_model_path(path: Option<&str>) -> Result<PathBuf, String> {
    let raw = path
    .map(str::trim)
    .filter(|v| !v.is_empty())
    .ok_or_else(|| {
      "WHISPER_MODEL_PATH is not set. Point it to a local GGML Whisper model file (example: ggml-base.en.bin).".to_string()
    })?;

    let model_path = PathBuf::from(raw);
    if !model_path.exists() {
        return Err(format!(
            "WHISPER_MODEL_PATH file not found: {}",
            model_path.display()
        ));
    }

    Ok(model_path)
}

fn resolve_whisper_cli_path(path: Option<&str>) -> String {
    path.map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("whisper-cli")
        .to_string()
}

fn ensure_whisper_cli_available(whisper_cli_path: &str) -> Result<(), String> {
    Command::new(whisper_cli_path)
    .arg("--help")
    .output()
    .map(|_| ())
    .map_err(|e| {
      format!(
        "Could not execute '{whisper_cli_path}': {e}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
      )
    })
}

fn push_downmixed<T, F>(data: &[T], channels: usize, target: &Arc<Mutex<Vec<f32>>>, to_f32: F)
where
    T: Copy,
    F: Fn(T) -> f32,
{
    if channels == 0 || data.is_empty() {
        return;
    }

    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
    for frame in data.chunks(channels) {
        let sum: f32 = frame.iter().map(|sample| to_f32(*sample)).sum();
        mono.push(sum / frame.len() as f32);
    }

    if let Ok(mut guard) = target.lock() {
        guard.extend(mono);
    }
}

fn create_input_stream(samples: Arc<Mutex<Vec<f32>>>) -> Result<(Stream, u32), String> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| {
        "No microphone input device found. Check macOS input device settings.".to_string()
    })?;

    let supported_config = device
        .default_input_config()
        .map_err(|e| format!("Failed to read default input config: {e}"))?;
    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels() as usize;
    let config: cpal::StreamConfig = supported_config.clone().into();
    let err_fn = |err| {
        eprintln!("microphone stream error: {err}");
    };

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => {
            let sink = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        push_downmixed(data, channels, &sink, |v| v);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open f32 input stream: {e}"))?
        }
        SampleFormat::I16 => {
            let sink = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        push_downmixed(data, channels, &sink, |v| v as f32 / i16::MAX as f32);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open i16 input stream: {e}"))?
        }
        SampleFormat::U16 => {
            let sink = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        push_downmixed(data, channels, &sink, |v| {
                            (v as f32 / u16::MAX as f32) * 2.0 - 1.0
                        });
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open u16 input stream: {e}"))?
        }
        sample_format => {
            return Err(format!(
                "Unsupported input sample format: {sample_format:?}. Try a different input device."
            ));
        }
    };

    Ok((stream, sample_rate))
}

fn spawn_recording_thread(
    samples: Arc<Mutex<Vec<f32>>>,
) -> Result<(mpsc::Sender<()>, thread::JoinHandle<()>, u32), String> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (init_tx, init_rx) = mpsc::channel::<Result<u32, String>>();
    let capture_samples = Arc::clone(&samples);

    let handle = thread::spawn(move || {
        let stream_result = create_input_stream(capture_samples);
        match stream_result {
            Ok((stream, sample_rate)) => match stream.play() {
                Ok(()) => {
                    let _ = init_tx.send(Ok(sample_rate));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = init_tx.send(Err(format!("Failed to start microphone stream: {e}")));
                }
            },
            Err(e) => {
                let _ = init_tx.send(Err(e));
            }
        }
    });

    let sample_rate = match init_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(rate)) => rate,
        Ok(Err(e)) => {
            let _ = handle.join();
            return Err(e);
        }
        Err(_) => {
            let _ = stop_tx.send(());
            let _ = handle.join();
            return Err("Timed out while opening microphone stream.".to_string());
        }
    };

    Ok((stop_tx, handle, sample_rate))
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

fn transcribe_samples(
    model_path: PathBuf,
    whisper_cli_path: String,
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<String, String> {
    let prepared = if sample_rate == WHISPER_SAMPLE_RATE {
        samples
    } else {
        resample_linear(&samples, sample_rate, WHISPER_SAMPLE_RATE)
    };

    if prepared.is_empty() {
        return Err("No audio captured. Check microphone input and try again.".to_string());
    }

    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir();
    let base_name = format!("dicktaint-{}-{tick}", std::process::id());
    let wav_path = temp_dir.join(format!("{base_name}.wav"));
    let out_prefix = temp_dir.join(format!("{base_name}-transcript"));
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
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_file(&txt_path);
        return Err(format!("whisper-cli transcription failed: {detail}"));
    }

    let transcript = std::fs::read_to_string(&txt_path).map_err(|e| {
        format!(
            "whisper-cli ran but transcript file is missing at {}: {e}",
            txt_path.display()
        )
    })?;

    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(&txt_path);

    let cleaned = transcript.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return Err("No speech detected in the recorded audio.".to_string());
    }

    Ok(cleaned)
}

#[tauri::command]
async fn list_models(state: State<'_, AppConfig>) -> Result<Vec<String>, String> {
    let client = Client::new();
    let url = format!("{}/api/tags", normalize_host(&state.ollama_host));

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama /api/tags returned {}", response.status()));
    }

    let payload = response
        .json::<TagsResponse>()
        .await
        .map_err(|e| format!("Invalid Ollama response: {e}"))?;

    Ok(payload.models.into_iter().map(|m| m.name).collect())
}

#[tauri::command]
fn start_native_dictation(
    dictation: State<'_, DictationState>,
    config: State<'_, AppConfig>,
) -> Result<(), String> {
    resolve_whisper_model_path(config.whisper_model_path.as_deref())?;
    let whisper_cli_path = resolve_whisper_cli_path(config.whisper_cli_path.as_deref());
    ensure_whisper_cli_available(&whisper_cli_path)?;

    let mut guard = dictation
        .inner()
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())?;
    if guard.is_some() {
        return Err("Dictation already running.".to_string());
    }

    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let (stop_tx, thread_handle, sample_rate) = spawn_recording_thread(Arc::clone(&samples))?;
    *guard = Some(ActiveRecording {
        stop_tx,
        thread_handle,
        samples,
        sample_rate,
    });
    Ok(())
}

#[tauri::command]
async fn stop_native_dictation(
    dictation: State<'_, DictationState>,
    config: State<'_, AppConfig>,
) -> Result<String, String> {
    let recording = {
        let mut guard = dictation
            .inner()
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard
            .take()
            .ok_or_else(|| "Dictation is not running.".to_string())?
    };

    let _ = recording.stop_tx.send(());
    if recording.thread_handle.join().is_err() {
        return Err("Audio capture thread crashed.".to_string());
    }

    let captured_samples = recording
        .samples
        .lock()
        .map_err(|_| "Failed to read captured audio".to_string())?
        .clone();
    let model_path = resolve_whisper_model_path(config.whisper_model_path.as_deref())?;
    let whisper_cli_path = resolve_whisper_cli_path(config.whisper_cli_path.as_deref());

    tauri::async_runtime::spawn_blocking(move || {
        transcribe_samples(
            model_path,
            whisper_cli_path,
            captured_samples,
            recording.sample_rate,
        )
    })
    .await
    .map_err(|e| format!("Failed to run transcription task: {e}"))?
}

#[tauri::command]
fn cancel_native_dictation(dictation: State<'_, DictationState>) -> Result<(), String> {
    let recording = {
        let mut guard = dictation
            .inner()
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard.take()
    };

    if let Some(recording) = recording {
        let _ = recording.stop_tx.send(());
        let _ = recording.thread_handle.join();
    }

    Ok(())
}

#[tauri::command]
async fn refine_dictation(
    model: String,
    transcript: String,
    instruction: Option<String>,
    state: State<'_, AppConfig>,
) -> Result<String, String> {
    let trimmed_model = model.trim().to_string();
    let trimmed_transcript = transcript.trim().to_string();

    if trimmed_model.is_empty() {
        return Err("Missing model".to_string());
    }
    if trimmed_transcript.is_empty() {
        return Err("Missing transcript".to_string());
    }

    let prompt_instruction = instruction
    .as_deref()
    .map(str::trim)
    .filter(|v| !v.is_empty())
    .unwrap_or(
      "Clean up this raw speech-to-text transcript into readable text while preserving the speaker's intent.",
    );

    let payload = GenerateRequest {
        model: trimmed_model,
        prompt: make_prompt(&trimmed_transcript, prompt_instruction),
        stream: false,
        options: GenerateOptions { temperature: 0.2 },
    };

    let client = Client::new();
    let url = format!("{}/api/generate", normalize_host(&state.ollama_host));
    let response = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(format!("Ollama /api/generate failed ({status}): {body}"));
    }

    let generated = response
        .json::<GenerateResponse>()
        .await
        .map_err(|e| format!("Invalid Ollama response: {e}"))?;

    Ok(generated.response.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::{make_prompt, normalize_host, resample_linear};

    #[test]
    fn make_prompt_includes_instruction_and_transcript() {
        let prompt = make_prompt("raw words here", "Clean this please");
        assert!(prompt.contains("Clean this please"));
        assert!(prompt.contains("Raw transcript:"));
        assert!(prompt.contains("raw words here"));
        assert!(prompt.contains("Output only the cleaned dictation text."));
    }

    #[test]
    fn normalize_host_trims_only_trailing_slash() {
        assert_eq!(
            normalize_host("http://127.0.0.1:11434/"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            normalize_host("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
    }

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
}

fn main() {
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let whisper_model_path = std::env::var("WHISPER_MODEL_PATH").ok();
    let whisper_cli_path = std::env::var("WHISPER_CLI_PATH").ok();

    tauri::Builder::default()
        .manage(AppConfig {
            ollama_host,
            whisper_model_path,
            whisper_cli_path,
        })
        .manage(DictationState::default())
        .invoke_handler(tauri::generate_handler![
            list_models,
            start_native_dictation,
            stop_native_dictation,
            cancel_native_dictation,
            refine_dictation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
