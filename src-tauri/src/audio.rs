//! Microphone capture, input device selection, and live audio metering.

use crate::hotkey_overlay::show_main_window;
use crate::state::{
    AudioSignalStats, DictationAudioLevelPayload, DictationInputDevice, LiveAudioMeter,
    LocalModelState, DICTATION_AUDIO_LEVEL_EVENT, INPUT_STREAM_PROBE_POLL_INTERVAL_MS,
    INPUT_STREAM_PROBE_TIMEOUT_MS, LIVE_AUDIO_BAR_COUNT, LIVE_AUDIO_EMIT_INTERVAL_MS,
};
use crate::transcribe::analyze_audio_signal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
#[cfg(target_os = "macos")]
use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};


fn downmix_samples<T, F>(data: &[T], channels: usize, to_f32: F) -> Vec<f32>
where
    T: Copy,
    F: Fn(T) -> f32,
{
    if channels == 0 || data.is_empty() {
        return Vec::new();
    }

    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
    for frame in data.chunks(channels) {
        let sum: f32 = frame.iter().map(|sample| to_f32(*sample)).sum();
        mono.push(sum / frame.len() as f32);
    }

    mono
}

fn store_captured_samples(target: &Arc<Mutex<Vec<f32>>>, samples: &[f32]) {
    if samples.is_empty() {
        return;
    }

    if let Ok(mut guard) = target.lock() {
        guard.extend_from_slice(samples);
    }
}

fn audio_level_from_stats(stats: AudioSignalStats) -> f32 {
    let peak = (stats.peak_abs / 0.18).clamp(0.0, 1.0);
    let rms = (stats.rms / 0.06).clamp(0.0, 1.0);
    ((peak * 0.68) + (rms * 0.32)).clamp(0.0, 1.0)
}

fn waveform_bins_from_samples(samples: &[f32], count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    if samples.is_empty() {
        return vec![0.0; count];
    }

    let chunk_len = (samples.len() / count).max(1);
    let mut bins = Vec::with_capacity(count);
    for index in 0..count {
        let start = index * chunk_len;
        let end = ((index + 1) * chunk_len).min(samples.len());
        let slice = if start < samples.len() {
            &samples[start..end.max(start + 1).min(samples.len())]
        } else {
            &samples[samples.len().saturating_sub(1)..]
        };
        let peak = slice
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        let normalized = (peak / 0.18).sqrt().clamp(0.0, 1.0);
        bins.push(normalized);
    }

    bins
}

impl LiveAudioMeter {
    fn emit_samples(&self, samples: &[f32], sample_rate: u32) {
        if samples.is_empty() || sample_rate == 0 {
            return;
        }

        let now = Instant::now();
        let should_emit = if let Ok(mut guard) = self.last_emitted_at.lock() {
            match *guard {
                Some(last)
                    if now.duration_since(last)
                        < Duration::from_millis(LIVE_AUDIO_EMIT_INTERVAL_MS) =>
                {
                    false
                }
                _ => {
                    *guard = Some(now);
                    true
                }
            }
        } else {
            false
        };

        if !should_emit {
            return;
        }

        let stats = analyze_audio_signal(samples, sample_rate);
        let payload = DictationAudioLevelPayload {
            session_id: self.session_id,
            peak_abs: stats.peak_abs,
            rms: stats.rms,
            level: audio_level_from_stats(stats),
            bars: waveform_bins_from_samples(samples, LIVE_AUDIO_BAR_COUNT),
        };
        self.app.emit(DICTATION_AUDIO_LEVEL_EVENT, payload).ok();
    }
}

fn handle_input_chunk<T, F>(
    data: &[T],
    channels: usize,
    target: &Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    meter: &LiveAudioMeter,
    to_f32: F,
) where
    T: Copy,
    F: Fn(T) -> f32,
{
    let mono = downmix_samples(data, channels, to_f32);
    if mono.is_empty() {
        return;
    }

    store_captured_samples(target, &mono);
    meter.emit_samples(&mono, sample_rate);
}

fn sample_format_rank(sample_format: SampleFormat) -> u8 {
    match sample_format {
        SampleFormat::F32 => 3,
        SampleFormat::I16 => 2,
        SampleFormat::U16 => 1,
        _ => 0,
    }
}

fn choose_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, String> {
    if let Ok(default_config) = device.default_input_config() {
        return Ok(default_config);
    }

    let mut best: Option<(u8, u32, cpal::SupportedStreamConfig)> = None;
    let ranges = device
        .supported_input_configs()
        .map_err(|e| format!("Failed to query supported input configs: {e}"))?;

    for range in ranges {
        let candidate = range.with_max_sample_rate();
        let format_rank = sample_format_rank(candidate.sample_format());
        if format_rank == 0 {
            continue;
        }
        let candidate_rate = candidate.sample_rate().0;

        let replace = match &best {
            Some((best_rank, best_rate, _)) => {
                format_rank > *best_rank
                    || (format_rank == *best_rank && candidate_rate > *best_rate)
            }
            None => true,
        };
        if replace {
            best = Some((format_rank, candidate_rate, candidate));
        }
    }

    best.map(|(_, _, config)| config).ok_or_else(|| {
        "No compatible microphone input config found. Try a different input device.".to_string()
    })
}

fn device_name(device: &cpal::Device, fallback: &str) -> String {
    device.name().unwrap_or_else(|_| fallback.to_string())
}

pub(crate) fn list_input_devices() -> Vec<DictationInputDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .map(|device| device_name(&device, "default input"));
    let mut devices = Vec::<DictationInputDevice>::new();
    let mut seen_names = HashSet::<String>::new();

    if let Some(name) = default_name.clone() {
        seen_names.insert(name.clone());
        devices.push(DictationInputDevice {
            is_default: true,
            name,
        });
    }

    if let Ok(inputs) = host.input_devices() {
        for device in inputs {
            let name = device_name(&device, "unknown input");
            if !seen_names.insert(name.clone()) {
                continue;
            }
            devices.push(DictationInputDevice {
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
            });
        }
    }

    devices
}

fn create_input_stream_for_device(
    device: &cpal::Device,
    device_name: &str,
    samples: Arc<Mutex<Vec<f32>>>,
    meter: LiveAudioMeter,
) -> Result<(Stream, u32), String> {
    let supported_config = device
        .default_input_config()
        .or_else(|_| choose_input_config(device))
        .map_err(|e| format!("Failed to resolve input config: {e}"))?;
    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels() as usize;
    let config: cpal::StreamConfig = supported_config.clone().into();
    let probe_start_len = samples.lock().map(|guard| guard.len()).unwrap_or(0);
    let err_fn = |err| {
        eprintln!("microphone stream error: {err}");
    };

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => {
            let sink = Arc::clone(&samples);
            let live_meter = meter.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        handle_input_chunk(data, channels, &sink, sample_rate, &live_meter, |v| v);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open f32 input stream: {e}"))?
        }
        SampleFormat::I16 => {
            let sink = Arc::clone(&samples);
            let live_meter = meter.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        handle_input_chunk(data, channels, &sink, sample_rate, &live_meter, |v| {
                            v as f32 / i16::MAX as f32
                        });
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open i16 input stream: {e}"))?
        }
        SampleFormat::U16 => {
            let sink = Arc::clone(&samples);
            let live_meter = meter.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        handle_input_chunk(data, channels, &sink, sample_rate, &live_meter, |v| {
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

    stream
        .play()
        .map_err(|e| format!("Failed to start microphone stream: {e}"))?;

    if let Err(error) =
        wait_for_non_silent_input(&samples, probe_start_len, sample_rate, device_name)
    {
        if let Ok(mut guard) = samples.lock() {
            guard.truncate(probe_start_len);
        }
        stop_and_drop_input_stream(stream);
        return Err(error);
    }

    Ok((stream, sample_rate))
}

fn stop_and_drop_input_stream(stream: Stream) {
    if let Err(error) = stream.pause() {
        log::warn!("Failed to pause microphone stream before drop: {error}");
    }
    drop(stream);
}

fn ordered_input_device_candidate_names(
    preferred_input_name: Option<&str>,
    default_name: Option<&str>,
    available_names: &[String],
) -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();

    let mut push_name = |name: &str| {
        if seen.insert(name.to_string()) {
            ordered.push(name.to_string());
        }
    };

    if let (Some(preferred_name), Some(default_name)) = (preferred_input_name, default_name) {
        if preferred_name == default_name {
            push_name(default_name);
        }
    }

    if let Some(preferred_name) = preferred_input_name {
        for name in available_names {
            if name == preferred_name {
                push_name(name);
            }
        }
    }

    if let Some(default_name) = default_name {
        push_name(default_name);
    }

    for name in available_names {
        push_name(name);
    }

    ordered
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputStreamProbeOutcome {
    NonSilentFrames,
    SilentFrames,
}

fn probe_input_stream_activity(
    samples: &Arc<Mutex<Vec<f32>>>,
    start_len: usize,
    sample_rate: u32,
    device_name: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<InputStreamProbeOutcome, String> {
    if sample_rate == 0 {
        return Ok(InputStreamProbeOutcome::NonSilentFrames);
    }

    let deadline = Instant::now() + timeout;
    let mut saw_any_frames = false;

    loop {
        let observed = if let Ok(guard) = samples.lock() {
            if guard.len() <= start_len {
                None
            } else {
                let captured = &guard[start_len..];
                Some((
                    captured.len(),
                    analyze_audio_signal(captured, sample_rate).peak_abs,
                ))
            }
        } else {
            None
        };

        if let Some((captured_len, peak_abs)) = observed {
            saw_any_frames = true;
            if peak_abs > 0.0 {
                return Ok(InputStreamProbeOutcome::NonSilentFrames);
            }
            let _ = captured_len;
        }

        if Instant::now() >= deadline {
            if !saw_any_frames {
                return Err(format!(
                    "Microphone '{}' did not deliver any audio frames after opening. In macOS Settings > Privacy & Security > Microphone, allow this app and retry.",
                    device_name
                ));
            }
            return Ok(InputStreamProbeOutcome::SilentFrames);
        }

        thread::sleep(poll_interval);
    }
}

fn wait_for_non_silent_input(
    samples: &Arc<Mutex<Vec<f32>>>,
    start_len: usize,
    sample_rate: u32,
    device_name: &str,
) -> Result<(), String> {
    match probe_input_stream_activity(
        samples,
        start_len,
        sample_rate,
        device_name,
        Duration::from_millis(INPUT_STREAM_PROBE_TIMEOUT_MS),
        Duration::from_millis(INPUT_STREAM_PROBE_POLL_INTERVAL_MS),
    )? {
        InputStreamProbeOutcome::NonSilentFrames => Ok(()),
        InputStreamProbeOutcome::SilentFrames => {
            log::warn!(
                "Microphone '{}' opened with only silent startup frames; continuing because macOS Mic Mode can suppress initial silence.",
                device_name
            );
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn microphone_media_type() -> Result<&'static objc2_av_foundation::AVMediaType, String> {
    unsafe {
        AVMediaTypeAudio.ok_or_else(|| {
            "AVFoundation did not expose AVMediaTypeAudio on this macOS build.".to_string()
        })
    }
}

#[cfg(target_os = "macos")]
fn microphone_permission_denied_error() -> String {
    "Microphone permission is denied for this app. In macOS Settings > Privacy & Security > Microphone, allow dicktaint and relaunch the app.".to_string()
}

#[cfg(target_os = "macos")]
fn microphone_permission_restricted_error() -> String {
    "Microphone access is restricted by macOS for this app. Check Privacy & Security > Microphone or system policy restrictions and retry.".to_string()
}

#[cfg(target_os = "macos")]
fn should_focus_main_window_for_microphone_prompt(status: AVAuthorizationStatus) -> bool {
    status == AVAuthorizationStatus::NotDetermined
}

#[cfg(target_os = "macos")]
pub(crate) fn ensure_microphone_access_authorized(app: &tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let tx_main = tx.clone();
    let app_handle = app.clone();

    app.run_on_main_thread(move || {
        let media_type = match microphone_media_type() {
            Ok(value) => value,
            Err(error) => {
                let _ = tx_main.send(Err(error));
                return;
            }
        };

        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };
        if status == AVAuthorizationStatus::Authorized {
            let _ = tx_main.send(Ok(()));
            return;
        }
        if status == AVAuthorizationStatus::Denied {
            let _ = tx_main.send(Err(microphone_permission_denied_error()));
            return;
        }
        if status == AVAuthorizationStatus::Restricted {
            let _ = tx_main.send(Err(microphone_permission_restricted_error()));
            return;
        }
        if !should_focus_main_window_for_microphone_prompt(status) {
            let _ = tx_main.send(Err(format!(
                "Microphone access returned an unknown AVFoundation authorization state ({}).",
                status.0
            )));
            return;
        }

        show_main_window(&app_handle);

        let tx_request = tx_main.clone();
        let handler = RcBlock::new(move |granted| {
            let _ = tx_request.send(if bool::from(granted) {
                Ok(())
            } else {
                Err(microphone_permission_denied_error())
            });
        });
        unsafe {
            AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &handler);
        }
    })
    .map_err(|e| format!("Failed to request microphone access on the macOS main thread: {e}"))?;

    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(result) => result,
        Err(_) => Err(
            "Timed out waiting for macOS microphone permission. Bring dicktaint to the foreground, approve access in Privacy & Security > Microphone, then retry."
                .to_string(),
        ),
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ensure_microphone_access_authorized(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}
fn create_input_stream(
    samples: Arc<Mutex<Vec<f32>>>,
    meter: LiveAudioMeter,
) -> Result<(Stream, u32, String), String> {
    let host = cpal::default_host();
    let preferred_input_name = meter
        .app
        .state::<LocalModelState>()
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .preferred_input_device
        .clone();
    let mut candidate_devices: Vec<(String, cpal::Device)> = Vec::new();
    let mut enumerated_devices: Vec<(String, cpal::Device)> = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            enumerated_devices.push((device_name(&device, "unknown input"), device));
        }
    }

    let mut default_device = host
        .default_input_device()
        .map(|device| (device_name(&device, "default input"), device));
    let default_name = default_device.as_ref().map(|(name, _)| name.clone());
    let available_names = enumerated_devices
        .iter()
        .map(|(name, _)| name.clone())
        .chain(default_name.iter().cloned())
        .collect::<Vec<_>>();

    for ordered_name in ordered_input_device_candidate_names(
        preferred_input_name.as_deref(),
        default_name.as_deref(),
        &available_names,
    ) {
        if default_device
            .as_ref()
            .is_some_and(|(name, _)| *name == ordered_name)
        {
            if let Some(device) = default_device.take() {
                candidate_devices.push(device);
                continue;
            }
        }

        if let Some(index) = enumerated_devices
            .iter()
            .position(|(name, _)| *name == ordered_name)
        {
            candidate_devices.push(enumerated_devices.remove(index));
        }
    }

    if candidate_devices.is_empty() {
        return Err(
            "No microphone input device found. In macOS Settings > Sound > Input, select a microphone and retry."
                .to_string(),
        );
    }

    let mut attempts: Vec<String> = Vec::new();
    for (name, device) in candidate_devices {
        match create_input_stream_for_device(&device, &name, Arc::clone(&samples), meter.clone()) {
            Ok((stream, sample_rate)) => return Ok((stream, sample_rate, name)),
            Err(err) => attempts.push(format!("{name}: {err}")),
        }
    }

    let preferred_detail = preferred_input_name
        .as_deref()
        .map(|name| format!(" Preferred input: {name}."))
        .unwrap_or_default();
    let default_detail = default_name
        .as_deref()
        .map(|name| format!(" Default input: {name}."))
        .unwrap_or_default();

    Err(format!(
        "Could not open microphone input on this machine. Tried: {}. \
In macOS Settings > Privacy & Security > Microphone, allow this app/terminal, then pick an input device in Settings > Sound > Input and retry.{}{}",
        attempts.join(" | "),
        preferred_detail,
        default_detail
    ))
}

pub(crate) fn spawn_recording_thread(
    samples: Arc<Mutex<Vec<f32>>>,
    app: tauri::AppHandle,
    session_id: u64,
) -> Result<(mpsc::Sender<()>, thread::JoinHandle<()>, u32, String), String> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (init_tx, init_rx) = mpsc::channel::<Result<(u32, String), String>>();
    let capture_samples = Arc::clone(&samples);
    let meter = LiveAudioMeter {
        app,
        session_id,
        last_emitted_at: Arc::new(Mutex::new(None)),
    };

    let handle = thread::spawn(move || {
        let stream_result = create_input_stream(capture_samples, meter);
        match stream_result {
            Ok((stream, sample_rate, input_device_name)) => {
                let _ = init_tx.send(Ok((sample_rate, input_device_name)));
                let _ = stop_rx.recv();
                stop_and_drop_input_stream(stream);
            }
            Err(e) => {
                let _ = init_tx.send(Err(e));
            }
        }
    });

    let (sample_rate, input_device_name) = match init_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(value)) => value,
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

    Ok((stop_tx, handle, sample_rate, input_device_name))
}



#[cfg(test)]
mod tests {
    use super::{
        ordered_input_device_candidate_names, probe_input_stream_activity,
        wait_for_non_silent_input, InputStreamProbeOutcome,
    };
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[cfg(target_os = "macos")]
    use super::should_focus_main_window_for_microphone_prompt;
    #[cfg(target_os = "macos")]
    use objc2_av_foundation::AVAuthorizationStatus;

    #[test]
    fn silent_stream_probe_accepts_zeroed_frames_when_frames_exist() {
        let samples = Arc::new(Mutex::new(vec![0.0_f32; 4096]));
        let outcome = probe_input_stream_activity(
            &samples,
            0,
            16_000,
            "Austin's AirPods",
            Duration::ZERO,
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(outcome, InputStreamProbeOutcome::SilentFrames);
        wait_for_non_silent_input(&samples, 0, 16_000, "Austin's AirPods").unwrap();
    }

    #[test]
    fn silent_stream_probe_accepts_nonzero_frames() {
        let samples = Arc::new(Mutex::new(vec![0.0_f32, 0.02, -0.01, 0.0]));
        let outcome = probe_input_stream_activity(
            &samples,
            0,
            16_000,
            "MacBook Pro Microphone",
            Duration::ZERO,
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(outcome, InputStreamProbeOutcome::NonSilentFrames);
        wait_for_non_silent_input(&samples, 0, 16_000, "MacBook Pro Microphone").unwrap();
    }

    #[test]
    fn silent_stream_probe_rejects_missing_frames() {
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let error = probe_input_stream_activity(
            &samples,
            0,
            16_000,
            "MacBook Pro Microphone",
            Duration::ZERO,
            Duration::ZERO,
        )
        .unwrap_err();
        assert!(error.contains("did not deliver any audio frames"));
    }

    #[test]
    fn preferred_default_input_uses_default_handle_and_dedupes_names() {
        let ordered = ordered_input_device_candidate_names(
            Some("MacBook Pro Microphone"),
            Some("MacBook Pro Microphone"),
            &[
                "MacBook Pro Microphone".to_string(),
                "USB Mic".to_string(),
                "MacBook Pro Microphone".to_string(),
            ],
        );

        assert_eq!(
            ordered,
            vec![
                "MacBook Pro Microphone".to_string(),
                "USB Mic".to_string()
            ]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn microphone_permission_prompt_only_focuses_for_not_determined_status() {
        assert!(should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::NotDetermined
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Authorized
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Denied
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Restricted
        ));
    }
}
