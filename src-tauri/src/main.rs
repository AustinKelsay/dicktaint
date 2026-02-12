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
use tauri::{Manager, State};

const WHISPER_SAMPLE_RATE: u32 = 16_000;
const APP_SETTINGS_DIR: &str = ".dicktaint";
const APP_SETTINGS_FILE: &str = "dictation-settings.json";
const APP_MODELS_DIR: &str = "whisper-models";
const DEFAULT_WHISPER_CLI_PATH: &str = "whisper-cli";
const WHISPER_CPP_SETUP_URL: &str = "https://github.com/ggml-org/whisper.cpp#quick-start";

#[derive(Clone)]
struct AppConfig {
    ollama_host: String,
    whisper_model_path_override: Option<String>,
    whisper_cli_path_override: Option<String>,
    bundled_whisper_cli_path: Option<String>,
}

#[derive(Default)]
struct DictationState {
    active_recording: Mutex<Option<ActiveRecording>>,
}

#[derive(Clone, Copy)]
struct WhisperModelSpec {
    id: &'static str,
    display_name: &'static str,
    whisper_ref: &'static str,
    file_name: &'static str,
    approx_size_gb: f32,
    min_ram_gb: u64,
    recommended_ram_gb: u64,
    speed_note: &'static str,
    quality_note: &'static str,
}

const WHISPER_MODEL_CATALOG: [WhisperModelSpec; 12] = [
    WhisperModelSpec {
        id: "tiny-en",
        display_name: "Whisper Tiny (English)",
        whisper_ref: "tiny.en",
        file_name: "ggml-tiny.en.bin",
        approx_size_gb: 0.08,
        min_ram_gb: 4,
        recommended_ram_gb: 8,
        speed_note: "Fastest",
        quality_note: "Lowest accuracy",
    },
    WhisperModelSpec {
        id: "tiny",
        display_name: "Whisper Tiny (Multilingual)",
        whisper_ref: "tiny",
        file_name: "ggml-tiny.bin",
        approx_size_gb: 0.15,
        min_ram_gb: 6,
        recommended_ram_gb: 8,
        speed_note: "Very fast",
        quality_note: "Low accuracy",
    },
    WhisperModelSpec {
        id: "base-en",
        display_name: "Whisper Base (English)",
        whisper_ref: "base.en",
        file_name: "ggml-base.en.bin",
        approx_size_gb: 0.15,
        min_ram_gb: 6,
        recommended_ram_gb: 10,
        speed_note: "Fast",
        quality_note: "Balanced",
    },
    WhisperModelSpec {
        id: "base",
        display_name: "Whisper Base (Multilingual)",
        whisper_ref: "base",
        file_name: "ggml-base.bin",
        approx_size_gb: 0.29,
        min_ram_gb: 8,
        recommended_ram_gb: 12,
        speed_note: "Fast",
        quality_note: "Balanced multilingual",
    },
    WhisperModelSpec {
        id: "small-en",
        display_name: "Whisper Small (English)",
        whisper_ref: "small.en",
        file_name: "ggml-small.en.bin",
        approx_size_gb: 0.46,
        min_ram_gb: 8,
        recommended_ram_gb: 16,
        speed_note: "Medium",
        quality_note: "Better accuracy",
    },
    WhisperModelSpec {
        id: "small",
        display_name: "Whisper Small (Multilingual)",
        whisper_ref: "small",
        file_name: "ggml-small.bin",
        approx_size_gb: 0.93,
        min_ram_gb: 10,
        recommended_ram_gb: 18,
        speed_note: "Medium",
        quality_note: "Better multilingual accuracy",
    },
    WhisperModelSpec {
        id: "medium-en",
        display_name: "Whisper Medium (English)",
        whisper_ref: "medium.en",
        file_name: "ggml-medium.en.bin",
        approx_size_gb: 1.5,
        min_ram_gb: 16,
        recommended_ram_gb: 24,
        speed_note: "Slowest in starter set",
        quality_note: "Best accuracy in starter set",
    },
    WhisperModelSpec {
        id: "medium",
        display_name: "Whisper Medium (Multilingual)",
        whisper_ref: "medium",
        file_name: "ggml-medium.bin",
        approx_size_gb: 1.5,
        min_ram_gb: 18,
        recommended_ram_gb: 28,
        speed_note: "Slower",
        quality_note: "Strong multilingual accuracy",
    },
    WhisperModelSpec {
        id: "large-v1",
        display_name: "Whisper Large v1",
        whisper_ref: "large-v1",
        file_name: "ggml-large-v1.bin",
        approx_size_gb: 2.9,
        min_ram_gb: 24,
        recommended_ram_gb: 32,
        speed_note: "Heavy",
        quality_note: "High accuracy",
    },
    WhisperModelSpec {
        id: "large-v2",
        display_name: "Whisper Large v2",
        whisper_ref: "large-v2",
        file_name: "ggml-large-v2.bin",
        approx_size_gb: 2.9,
        min_ram_gb: 24,
        recommended_ram_gb: 32,
        speed_note: "Heavy",
        quality_note: "High accuracy",
    },
    WhisperModelSpec {
        id: "large-v3",
        display_name: "Whisper Large v3",
        whisper_ref: "large-v3",
        file_name: "ggml-large-v3.bin",
        approx_size_gb: 3.1,
        min_ram_gb: 32,
        recommended_ram_gb: 48,
        speed_note: "Heaviest",
        quality_note: "Top accuracy",
    },
    WhisperModelSpec {
        id: "turbo",
        display_name: "Whisper Turbo",
        whisper_ref: "turbo",
        file_name: "ggml-large-v3-turbo.bin",
        approx_size_gb: 1.62,
        min_ram_gb: 20,
        recommended_ram_gb: 32,
        speed_note: "Fast large-class",
        quality_note: "Great quality/speed tradeoff",
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
    settings: Arc<Mutex<LocalSettings>>,
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
    whisper_ref: String,
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
    whisper_cli_available: bool,
    whisper_cli_path: String,
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

fn resolve_whisper_cli_path(override_path: Option<&str>, bundled_path: Option<&str>) -> String {
    if let Some(path) = override_path.map(str::trim).filter(|v| !v.is_empty()) {
        return path.to_string();
    }
    if let Some(path) = bundled_path.map(str::trim).filter(|v| !v.is_empty()) {
        return path.to_string();
    }
    DEFAULT_WHISPER_CLI_PATH.to_string()
}

fn ensure_whisper_cli_available(whisper_cli_path: &str) -> Result<(), String> {
    let output = Command::new(whisper_cli_path)
        .arg("--help")
        .output()
        .map_err(|e| {
            format!(
                "Could not execute '{whisper_cli_path}': {e}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exited with status {}", output.status)
    };

    Err(format!(
        "Could not execute '{whisper_cli_path}': {detail}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
    ))
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

fn can_execute_command(executable: &str) -> bool {
    Command::new(executable)
        .arg("--help")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn is_whisper_cli_name(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        let lower = name.to_ascii_lowercase();
        return lower == "whisper-cli.exe"
            || (lower.starts_with("whisper-cli-") && lower.ends_with(".exe"));
    }
    name == "whisper-cli" || name.starts_with("whisper-cli-")
}

fn find_whisper_cli_in_dir(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name()?.to_str()?;
        if is_whisper_cli_name(name) {
            return Some(path);
        }
    }
    None
}

fn resolve_bundled_whisper_cli_path(app: &tauri::AppHandle) -> Option<String> {
    let mut candidate_dirs = Vec::<PathBuf>::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidate_dirs.push(resource_dir.clone());
        candidate_dirs.push(resource_dir.join("bin"));
        candidate_dirs.push(resource_dir.join("binaries"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidate_dirs.push(parent.to_path_buf());
            if cfg!(target_os = "macos") {
                candidate_dirs.push(parent.join("../Resources"));
                candidate_dirs.push(parent.join("../Resources/bin"));
            }
        }
    }

    let mut deduped = Vec::<PathBuf>::new();
    for dir in candidate_dirs {
        if !deduped.iter().any(|seen| seen == &dir) {
            deduped.push(dir);
        }
    }

    for dir in deduped {
        if let Some(path) = find_whisper_cli_in_dir(&dir) {
            return Some(path.to_string_lossy().to_string());
        }
    }

    None
}

fn candidate_whisper_cli_paths(configured_path: &str) -> Vec<String> {
    let mut candidates = Vec::<String>::new();

    if !configured_path.trim().is_empty() {
        candidates.push(configured_path.trim().to_string());
    }
    if configured_path.trim() != DEFAULT_WHISPER_CLI_PATH {
        candidates.push(DEFAULT_WHISPER_CLI_PATH.to_string());
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push("/opt/homebrew/bin/whisper-cli".to_string());
        candidates.push("/usr/local/bin/whisper-cli".to_string());
        candidates.push("/opt/homebrew/opt/whisper-cpp/bin/whisper-cli".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        candidates.push("/usr/local/bin/whisper-cli".to_string());
        candidates.push("/usr/bin/whisper-cli".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        candidates.push("C:\\Program Files\\whisper.cpp\\whisper-cli.exe".to_string());
        candidates.push("C:\\Program Files (x86)\\whisper.cpp\\whisper-cli.exe".to_string());
    }

    let mut deduped = Vec::new();
    for candidate in candidates {
        if !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn detect_whisper_cli_path(configured_path: &str) -> Option<String> {
    candidate_whisper_cli_paths(configured_path)
        .into_iter()
        .find(|candidate| can_execute_command(candidate))
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

fn whisper_model_catalog() -> &'static [WhisperModelSpec] {
    &WHISPER_MODEL_CATALOG
}

fn find_whisper_model_spec(id: &str) -> Option<WhisperModelSpec> {
    whisper_model_catalog()
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

fn model_path_for_spec(models_dir: &Path, spec: WhisperModelSpec) -> PathBuf {
    models_dir.join(spec.file_name)
}

fn model_fit_level(spec: WhisperModelSpec, total_memory_gb: u64) -> u8 {
    if total_memory_gb >= spec.recommended_ram_gb {
        2
    } else if total_memory_gb >= spec.min_ram_gb {
        1
    } else {
        0
    }
}

fn pick_recommended_model_id(total_memory_gb: u64) -> Option<&'static str> {
    whisper_model_catalog()
        .iter()
        .copied()
        .filter(|spec| model_fit_level(*spec, total_memory_gb) > 0)
        .max_by(|a, b| {
            let a_key = (
                model_fit_level(*a, total_memory_gb),
                a.recommended_ram_gb,
                a.approx_size_gb.to_bits(),
            );
            let b_key = (
                model_fit_level(*b, total_memory_gb),
                b.recommended_ram_gb,
                b.approx_size_gb.to_bits(),
            );
            a_key.cmp(&b_key)
        })
        .map(|spec| spec.id)
}

fn build_model_options(
    models_dir: &Path,
    total_memory_gb: u64,
    selected_model_id: Option<&str>,
) -> Vec<DictationModelOption> {
    let recommended_model_id = pick_recommended_model_id(total_memory_gb);

    whisper_model_catalog()
        .iter()
        .map(|spec| {
            let path = model_path_for_spec(models_dir, *spec);
            let installed = path.exists();
            let likely_runnable = total_memory_gb >= spec.min_ram_gb;
            let recommended = recommended_model_id.is_some_and(|id| id == spec.id);
            let is_selected = selected_model_id.is_some_and(|id| id == spec.id);

            DictationModelOption {
                id: spec.id.to_string(),
                display_name: if is_selected {
                    format!("{} (Selected)", spec.display_name)
                } else {
                    spec.display_name.to_string()
                },
                whisper_ref: spec.whisper_ref.to_string(),
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

fn download_whisper_model(model_spec: WhisperModelSpec, target_path: &Path) -> Result<(), String> {
    let target_str = target_path.to_string_lossy().to_string();
    let model_url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model_spec.file_name
    );

    #[cfg(target_os = "windows")]
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Invoke-WebRequest",
            "-Uri",
            &model_url,
            "-OutFile",
            &target_str,
        ])
        .output();

    #[cfg(not(target_os = "windows"))]
    let output = Command::new("curl")
        .args(["-L", "--fail", "--output", &target_str, &model_url])
        .output();

    match output {
        Ok(result) if result.status.success() && target_path.exists() => Ok(()),
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
            Err(format!(
                "Could not download whisper model '{}' from {}: {}",
                model_spec.id, model_url, detail
            ))
        }
        Err(e) => Err(format!(
            "Could not start model download command. Install curl or PowerShell support and retry: {e}"
        )),
    }
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
    let list_selected_model_id = if override_model_path.is_some() {
        None
    } else {
        settings.selected_model_id.as_deref()
    };
    let models = build_model_options(
        &model_state.models_dir,
        device.total_memory_gb,
        list_selected_model_id,
    );
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let detected_whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path);
    let whisper_cli_available = detected_whisper_cli_path.is_some();
    let onboarding_required = !selected_model_exists || !whisper_cli_available;

    Ok(DictationOnboardingPayload {
        onboarding_required,
        selected_model_id,
        selected_model_path,
        selected_model_exists,
        whisper_cli_available,
        whisper_cli_path: detected_whisper_cli_path.unwrap_or(configured_whisper_cli_path),
        models_dir: model_state.models_dir.to_string_lossy().to_string(),
        device,
        models,
    })
}

#[tauri::command]
fn open_whisper_setup_page() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(WHISPER_CPP_SETUP_URL);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(WHISPER_CPP_SETUP_URL);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", WHISPER_CPP_SETUP_URL]);
        cmd
    };

    let status = command
        .status()
        .map_err(|e| format!("Failed to open download page: {e}"))?;
    if !status.success() {
        return Err(format!(
            "Could not open setup page automatically. Open {WHISPER_CPP_SETUP_URL} manually."
        ));
    }

    Ok(())
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
async fn install_dictation_model(
    model: String,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<DictationModelSelection, String> {
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);
    ensure_whisper_cli_available(&whisper_cli_path)?;
    let trimmed_id = model.trim();
    if trimmed_id.is_empty() {
        return Err("Missing model id".to_string());
    }

    let model_spec = find_whisper_model_spec(trimmed_id).ok_or_else(|| {
        let ids = whisper_model_catalog()
            .iter()
            .map(|spec| spec.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("Unsupported dictation model '{trimmed_id}'. Available models: {ids}")
    })?;
    let models_dir = model_state.models_dir.clone();
    let settings_path = model_state.settings_path.clone();
    let settings = Arc::clone(&model_state.settings);

    let install_task =
        tauri::async_runtime::spawn_blocking(move || -> Result<DictationModelSelection, String> {
            fs::create_dir_all(&models_dir).map_err(|e| {
                format!(
                    "Failed to create model directory {}: {e}",
                    models_dir.display()
                )
            })?;

            let target_path = model_path_for_spec(&models_dir, model_spec);
            if !target_path.exists() {
                download_whisper_model(model_spec, &target_path)?;
                if !target_path.exists() {
                    return Err(format!(
                        "Model download completed but file is still missing at {}.",
                        target_path.display()
                    ));
                }
            }

            let selected_model_path = target_path.to_string_lossy().to_string();
            {
                let mut settings = settings
                    .lock()
                    .map_err(|_| "Failed to lock local model settings".to_string())?;
                settings.selected_model_id = Some(model_spec.id.to_string());
                settings.selected_model_path = Some(selected_model_path.clone());
                save_local_settings(&settings_path, &settings)?;
            }

            Ok(DictationModelSelection {
                selected_model_id: model_spec.id.to_string(),
                selected_model_path,
                installed: true,
            })
        });

    install_task
        .await
        .map_err(|e| format!("Model install task failed: {e}"))?
}

#[tauri::command]
fn start_native_dictation(
    dictation: State<'_, DictationState>,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<(), String> {
    resolve_active_model_path(config.inner(), model_state.inner())?;
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);
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
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);

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
    let whisper_cli_path_override = std::env::var("WHISPER_CLI_PATH").ok();

    tauri::Builder::default()
        .setup(move |app| {
            let bundled_whisper_cli_path = resolve_bundled_whisper_cli_path(app.handle());
            let (models_dir, settings_path) =
                resolve_local_paths().expect("failed to initialize local dictation model paths");
            let initial_settings = load_local_settings(&settings_path);

            app.manage(AppConfig {
                ollama_host: ollama_host.clone(),
                whisper_model_path_override: whisper_model_path_override.clone(),
                whisper_cli_path_override: whisper_cli_path_override.clone(),
                bundled_whisper_cli_path,
            });
            app.manage(LocalModelState {
                settings_path,
                models_dir,
                settings: Arc::new(Mutex::new(initial_settings)),
            });
            app.manage(DictationState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_models,
            get_dictation_onboarding,
            open_whisper_setup_page,
            install_dictation_model,
            start_native_dictation,
            stop_native_dictation,
            cancel_native_dictation,
            refine_dictation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
