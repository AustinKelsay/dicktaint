use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::State;

const WHISPER_SAMPLE_RATE: u32 = 16_000;
const APP_SETTINGS_DIR: &str = ".dicktaint";
const APP_SETTINGS_FILE: &str = "dictation-settings.json";
const APP_MODELS_DIR: &str = "wispr-models";
const DEFAULT_WISPR_CLI_PATH: &str = "wispr";

#[derive(Clone)]
struct AppConfig {
    ollama_host: String,
    whisper_model_path_override: Option<String>,
    whisper_cli_path: Option<String>,
    wispr_cli_path: String,
}

#[derive(Default)]
struct DictationState {
    active_recording: Mutex<Option<ActiveRecording>>,
}

#[derive(Clone, Copy)]
struct WisprModelSpec {
    id: &'static str,
    display_name: &'static str,
    wispr_ref: &'static str,
    file_name: &'static str,
    approx_size_gb: f32,
    min_ram_gb: u64,
    recommended_ram_gb: u64,
    speed_note: &'static str,
    quality_note: &'static str,
}

const WISPR_MODEL_CATALOG: [WisprModelSpec; 4] = [
    WisprModelSpec {
        id: "tiny-en",
        display_name: "Wispr Tiny (English)",
        wispr_ref: "tiny.en",
        file_name: "ggml-tiny.en.bin",
        approx_size_gb: 0.08,
        min_ram_gb: 4,
        recommended_ram_gb: 8,
        speed_note: "Fastest",
        quality_note: "Lowest accuracy",
    },
    WisprModelSpec {
        id: "base-en",
        display_name: "Wispr Base (English)",
        wispr_ref: "base.en",
        file_name: "ggml-base.en.bin",
        approx_size_gb: 0.15,
        min_ram_gb: 6,
        recommended_ram_gb: 10,
        speed_note: "Fast",
        quality_note: "Balanced",
    },
    WisprModelSpec {
        id: "small-en",
        display_name: "Wispr Small (English)",
        wispr_ref: "small.en",
        file_name: "ggml-small.en.bin",
        approx_size_gb: 0.46,
        min_ram_gb: 8,
        recommended_ram_gb: 16,
        speed_note: "Medium",
        quality_note: "Better accuracy",
    },
    WisprModelSpec {
        id: "medium-en",
        display_name: "Wispr Medium (English)",
        wispr_ref: "medium.en",
        file_name: "ggml-medium.en.bin",
        approx_size_gb: 1.5,
        min_ram_gb: 16,
        recommended_ram_gb: 24,
        speed_note: "Slowest in starter set",
        quality_note: "Best accuracy in starter set",
    },
];

#[derive(Default, Serialize, Deserialize, Clone)]
struct LocalSettings {
    selected_model_id: Option<String>,
    selected_model_path: Option<String>,
}

struct LocalModelState {
    settings_path: PathBuf,
    models_dir: PathBuf,
    settings: Mutex<LocalSettings>,
}

#[derive(Serialize)]
struct DeviceProfile {
    total_memory_gb: u64,
    logical_cpu_cores: usize,
    architecture: String,
    os: String,
}

#[derive(Serialize)]
struct DictationModelOption {
    id: String,
    display_name: String,
    wispr_ref: String,
    file_name: String,
    path: String,
    installed: bool,
    likely_runnable: bool,
    recommended: bool,
    approx_size_gb: f32,
    min_ram_gb: u64,
    recommended_ram_gb: u64,
    speed_note: String,
    quality_note: String,
}

#[derive(Serialize)]
struct DictationOnboardingPayload {
    onboarding_required: bool,
    selected_model_id: Option<String>,
    selected_model_path: Option<String>,
    selected_model_exists: bool,
    wispr_cli_available: bool,
    wispr_cli_path: String,
    models_dir: String,
    device: DeviceProfile,
    models: Vec<DictationModelOption>,
}

#[derive(Serialize)]
struct DictationModelSelection {
    selected_model_id: String,
    selected_model_path: String,
    installed: bool,
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

fn resolve_home_dir() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home));
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(home));
    }
    Err("Could not resolve user home directory for local model storage.".to_string())
}

fn resolve_local_paths() -> Result<(PathBuf, PathBuf), String> {
    let home = resolve_home_dir()?;
    let app_dir = home.join(APP_SETTINGS_DIR);
    let models_dir = app_dir.join(APP_MODELS_DIR);
    let settings_path = app_dir.join(APP_SETTINGS_FILE);

    fs::create_dir_all(&models_dir).map_err(|e| {
        format!(
            "Failed to create local model directory {}: {e}",
            models_dir.display()
        )
    })?;

    Ok((models_dir, settings_path))
}

fn load_local_settings(settings_path: &Path) -> LocalSettings {
    let raw = match fs::read_to_string(settings_path) {
        Ok(value) => value,
        Err(_) => return LocalSettings::default(),
    };

    serde_json::from_str::<LocalSettings>(&raw).unwrap_or_default()
}

fn save_local_settings(settings_path: &Path, settings: &LocalSettings) -> Result<(), String> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create settings directory {}: {e}",
                parent.display()
            )
        })?;
    }

    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize dictation settings: {e}"))?;
    fs::write(settings_path, serialized).map_err(|e| {
        format!(
            "Failed to write dictation settings file {}: {e}",
            settings_path.display()
        )
    })
}

fn wispr_model_catalog() -> &'static [WisprModelSpec] {
    &WISPR_MODEL_CATALOG
}

fn find_wispr_model_spec(id: &str) -> Option<WisprModelSpec> {
    wispr_model_catalog()
        .iter()
        .copied()
        .find(|spec| spec.id == id)
}

fn total_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return raw.parse::<u64>().ok();
    }

    #[cfg(target_os = "linux")]
    {
        let content = fs::read_to_string("/proc/meminfo").ok()?;
        let line = content
            .lines()
            .find(|entry| entry.starts_with("MemTotal:"))?;
        let kib = line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u64>().ok())?;
        return Some(kib.saturating_mul(1024));
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("wmic")
            .args(["ComputerSystem", "get", "TotalPhysicalMemory", "/Value"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let raw = String::from_utf8_lossy(&output.stdout);
        let value = raw
            .lines()
            .find_map(|line| line.strip_prefix("TotalPhysicalMemory="))?
            .trim()
            .to_string();
        return value.parse::<u64>().ok();
    }

    #[allow(unreachable_code)]
    None
}

fn system_memory_gb() -> u64 {
    let total_bytes = total_memory_bytes().unwrap_or(8 * 1_073_741_824);
    (((total_bytes as f64) / 1_073_741_824.0).round() as u64).max(1)
}

fn build_device_profile() -> DeviceProfile {
    let logical_cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    DeviceProfile {
        total_memory_gb: system_memory_gb(),
        logical_cpu_cores,
        architecture: std::env::consts::ARCH.to_string(),
        os: std::env::consts::OS.to_string(),
    }
}

fn model_path_for_spec(models_dir: &Path, spec: WisprModelSpec) -> PathBuf {
    models_dir.join(spec.file_name)
}

fn build_model_options(
    models_dir: &Path,
    total_memory_gb: u64,
    selected_model_id: Option<&str>,
) -> Vec<DictationModelOption> {
    wispr_model_catalog()
        .iter()
        .map(|spec| {
            let path = model_path_for_spec(models_dir, *spec);
            let installed = path.exists();
            let likely_runnable = total_memory_gb >= spec.min_ram_gb;
            let recommended = total_memory_gb >= spec.recommended_ram_gb;
            let is_selected = selected_model_id.is_some_and(|id| id == spec.id);

            DictationModelOption {
                id: spec.id.to_string(),
                display_name: if is_selected {
                    format!("{} (Selected)", spec.display_name)
                } else {
                    spec.display_name.to_string()
                },
                wispr_ref: spec.wispr_ref.to_string(),
                file_name: spec.file_name.to_string(),
                path: path.to_string_lossy().to_string(),
                installed,
                likely_runnable,
                recommended,
                approx_size_gb: spec.approx_size_gb,
                min_ram_gb: spec.min_ram_gb,
                recommended_ram_gb: spec.recommended_ram_gb,
                speed_note: spec.speed_note.to_string(),
                quality_note: spec.quality_note.to_string(),
            }
        })
        .collect()
}

fn resolve_active_model_path(
    config: &AppConfig,
    model_state: &LocalModelState,
) -> Result<PathBuf, String> {
    if let Some(path) = &config.whisper_model_path_override {
        return resolve_whisper_model_path(Some(path.as_str()));
    }

    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    let saved_path = settings
        .selected_model_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            "No local dictation model selected yet. Install one in onboarding before starting dictation."
                .to_string()
        })?;

    let path = PathBuf::from(saved_path);
    if !path.exists() {
        return Err(format!(
            "Selected dictation model file is missing: {}. Reinstall/select a model in onboarding.",
            path.display()
        ));
    }

    Ok(path)
}

fn command_render(exe: &str, args: &[&str]) -> String {
    let mut parts = vec![exe.to_string()];
    parts.extend(args.iter().map(|arg| (*arg).to_string()));
    parts.join(" ")
}

fn candidate_model_locations(file_name: &str) -> Vec<PathBuf> {
    let home = resolve_home_dir().ok();
    let mut paths = Vec::new();
    if let Some(home_dir) = home {
        paths.push(home_dir.join(".cache/wispr/models").join(file_name));
        paths.push(home_dir.join(".wispr/models").join(file_name));
        paths.push(home_dir.join(".local/share/wispr/models").join(file_name));
        paths.push(home_dir.join(".local/share/whisper-models").join(file_name));
    }
    paths
}

fn try_copy_from_common_model_locations(
    file_name: &str,
    target_path: &Path,
) -> Result<bool, String> {
    for candidate in candidate_model_locations(file_name) {
        if candidate.exists() {
            fs::copy(&candidate, target_path).map_err(|e| {
                format!(
                    "Model appeared in {} but could not be copied to {}: {e}",
                    candidate.display(),
                    target_path.display()
                )
            })?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn run_wispr_pull(
    wispr_cli_path: &str,
    model_spec: WisprModelSpec,
    target_path: &Path,
) -> Result<(), String> {
    let target_str = target_path.to_string_lossy().to_string();
    let pull_refs = [model_spec.wispr_ref, model_spec.file_name, model_spec.id];
    let mut attempts = Vec::new();

    for pull_ref in pull_refs {
        let arg_sets: [&[&str]; 6] = [
            &["pull", pull_ref, "--output", &target_str],
            &["model", "pull", pull_ref, "--output", &target_str],
            &["models", "pull", pull_ref, "--output", &target_str],
            &["pull", pull_ref],
            &["model", "pull", pull_ref],
            &["models", "pull", pull_ref],
        ];

        for args in arg_sets {
            let rendered = command_render(wispr_cli_path, args);
            let output = Command::new(wispr_cli_path).args(args).output();
            match output {
                Ok(result) if result.status.success() => {
                    if target_path.exists() {
                        return Ok(());
                    }

                    if try_copy_from_common_model_locations(model_spec.file_name, target_path)? {
                        return Ok(());
                    }

                    attempts.push(format!(
                        "{rendered} succeeded but no model file was found at {}",
                        target_path.display()
                    ));
                }
                Ok(result) => {
                    let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
                    let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
                    let detail = if !stderr.is_empty() {
                        stderr
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        "no output".to_string()
                    };
                    attempts.push(format!("{rendered} failed: {detail}"));
                }
                Err(e) => {
                    attempts.push(format!("{rendered} failed to start: {e}"));
                }
            }
        }
    }

    Err(format!(
        "Could not pull model via Wispr CLI. Tried command patterns for '{}'. Last errors:\n{}",
        model_spec.id,
        attempts
            .into_iter()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn build_onboarding_payload(
    config: &AppConfig,
    model_state: &LocalModelState,
) -> Result<DictationOnboardingPayload, String> {
    let device = build_device_profile();
    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    let override_model_path = config
        .whisper_model_path_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let override_model_exists = override_model_path
        .as_deref()
        .map(|value| Path::new(value).exists())
        .unwrap_or(false);

    let selected_model_path = if override_model_path.is_some() {
        override_model_path.clone()
    } else {
        settings.selected_model_path.clone()
    };
    let selected_model_exists = if override_model_path.is_some() {
        override_model_exists
    } else {
        settings
            .selected_model_path
            .as_deref()
            .map(|value| Path::new(value).exists())
            .unwrap_or(false)
    };
    let selected_model_id = if override_model_path.is_some() {
        Some("env-override".to_string())
    } else {
        settings.selected_model_id.clone()
    };
    let onboarding_required = !selected_model_exists;
    let models = build_model_options(
        &model_state.models_dir,
        device.total_memory_gb,
        settings.selected_model_id.as_deref(),
    );
    let wispr_cli_available = Command::new(&config.wispr_cli_path)
        .arg("--help")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    Ok(DictationOnboardingPayload {
        onboarding_required,
        selected_model_id,
        selected_model_path,
        selected_model_exists,
        wispr_cli_available,
        wispr_cli_path: config.wispr_cli_path.clone(),
        models_dir: model_state.models_dir.to_string_lossy().to_string(),
        device,
        models,
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
fn get_dictation_onboarding(
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<DictationOnboardingPayload, String> {
    build_onboarding_payload(config.inner(), model_state.inner())
}

#[tauri::command]
fn install_dictation_model(
    model: String,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<DictationModelSelection, String> {
    let trimmed_id = model.trim();
    if trimmed_id.is_empty() {
        return Err("Missing model id".to_string());
    }

    let model_spec = find_wispr_model_spec(trimmed_id).ok_or_else(|| {
        let ids = wispr_model_catalog()
            .iter()
            .map(|spec| spec.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("Unsupported dictation model '{trimmed_id}'. Available models: {ids}")
    })?;

    fs::create_dir_all(&model_state.models_dir).map_err(|e| {
        format!(
            "Failed to create model directory {}: {e}",
            model_state.models_dir.display()
        )
    })?;

    let target_path = model_path_for_spec(&model_state.models_dir, model_spec);
    if !target_path.exists() {
        run_wispr_pull(&config.wispr_cli_path, model_spec, &target_path)?;
        if !target_path.exists() {
            return Err(format!(
                "Wispr CLI completed but model file is still missing at {}.",
                target_path.display()
            ));
        }
    }

    let selected_model_path = target_path.to_string_lossy().to_string();
    {
        let mut settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        settings.selected_model_id = Some(model_spec.id.to_string());
        settings.selected_model_path = Some(selected_model_path.clone());
        save_local_settings(&model_state.settings_path, &settings)?;
    }

    Ok(DictationModelSelection {
        selected_model_id: model_spec.id.to_string(),
        selected_model_path,
        installed: true,
    })
}

#[tauri::command]
fn start_native_dictation(
    dictation: State<'_, DictationState>,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<(), String> {
    resolve_active_model_path(config.inner(), model_state.inner())?;
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
    model_state: State<'_, LocalModelState>,
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
    let model_path = resolve_active_model_path(config.inner(), model_state.inner())?;
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
    let whisper_model_path_override = std::env::var("WHISPER_MODEL_PATH").ok();
    let whisper_cli_path = std::env::var("WHISPER_CLI_PATH").ok();
    let wispr_cli_path =
        std::env::var("WISPR_CLI_PATH").unwrap_or_else(|_| DEFAULT_WISPR_CLI_PATH.to_string());
    let (models_dir, settings_path) =
        resolve_local_paths().expect("failed to initialize local dictation model paths");
    let initial_settings = load_local_settings(&settings_path);

    tauri::Builder::default()
        .manage(AppConfig {
            ollama_host,
            whisper_model_path_override,
            whisper_cli_path,
            wispr_cli_path,
        })
        .manage(LocalModelState {
            settings_path,
            models_dir,
            settings: Mutex::new(initial_settings),
        })
        .manage(DictationState::default())
        .invoke_handler(tauri::generate_handler![
            list_models,
            get_dictation_onboarding,
            install_dictation_model,
            start_native_dictation,
            stop_native_dictation,
            cancel_native_dictation,
            refine_dictation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
