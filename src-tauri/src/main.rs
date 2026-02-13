use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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

#[derive(Serialize)]
struct DictationModelDeletion {
    deleted_model_id: String,
    selected_model_id: Option<String>,
    selected_model_path: Option<String>,
}

struct ActiveRecording {
    stop_tx: mpsc::Sender<()>,
    thread_handle: thread::JoinHandle<()>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
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
    let preferred = if let Some(path) = override_path.map(str::trim).filter(|v| !v.is_empty()) {
        path.to_string()
    } else if let Some(path) = bundled_path.map(str::trim).filter(|v| !v.is_empty()) {
        path.to_string()
    } else {
        DEFAULT_WHISPER_CLI_PATH.to_string()
    };

    detect_whisper_cli_path(&preferred).unwrap_or(preferred)
}

fn ensure_whisper_cli_available(whisper_cli_path: &str) -> Result<(), String> {
    let executable = validate_whisper_cli_candidate(whisper_cli_path).map_err(|detail| {
        format!(
            "Could not execute '{whisper_cli_path}': {detail}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
        )
    })?;
    let output = run_help_probe(&executable).map_err(|e| {
        format!(
            "Could not execute '{whisper_cli_path}' (resolved to {}): {e}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH.",
            executable.display()
        )
    })?;
    if help_probe_looks_like_whisper_cli(&output) {
        return Ok(());
    }

    let probe_summary = help_probe_summary(&output);
    Err(format!(
        "Could not execute '{whisper_cli_path}' (resolved to {}): probe exited with status {} and did not return recognizable whisper-cli help output ({probe_summary}). Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH.",
        executable.display(),
        output.status
    ))
}

fn can_execute_command(executable: &str) -> bool {
    let path = match validate_whisper_cli_candidate(executable) {
        Ok(path) => path,
        Err(_) => return false,
    };
    run_help_probe(&path)
        .map(|output| help_probe_looks_like_whisper_cli(&output))
        .unwrap_or(false)
}

fn run_help_probe(executable: &Path) -> Result<Output, std::io::Error> {
    Command::new(executable).arg("--help").output()
}

fn help_probe_looks_like_whisper_cli(output: &Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    whisper_help_text_looks_valid(&stdout, &stderr)
}

fn help_probe_summary(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(line) = stderr.lines().map(str::trim).find(|line| !line.is_empty()) {
        return line.to_string();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = stdout.lines().map(str::trim).find(|line| !line.is_empty()) {
        return line.to_string();
    }

    "no output".to_string()
}

fn whisper_help_text_looks_valid(stdout: &str, stderr: &str) -> bool {
    let normalized = format!("{stdout}\n{stderr}").trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    if normalized.contains("placeholder")
        && normalized.contains("replace")
        && normalized.contains("whisper-cli")
    {
        return false;
    }

    let has_usage = normalized.contains("usage") || normalized.contains("options");
    let has_model_flag = normalized.contains("--model")
        || normalized.contains("\n-m ")
        || normalized.contains(" -m ");
    has_usage && has_model_flag
}

fn validate_whisper_cli_candidate(candidate: &str) -> Result<PathBuf, String> {
    let resolved_path = resolve_command_path(candidate).ok_or_else(|| {
        if is_explicit_path(candidate) {
            format!(
                "whisper-cli file not found at {}",
                Path::new(candidate).display()
            )
        } else {
            format!("whisper-cli command '{candidate}' was not found in PATH")
        }
    })?;

    let metadata = fs::metadata(&resolved_path).map_err(|e| {
        format!(
            "failed to read file metadata for {}: {e}",
            resolved_path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "{} exists but is not a file",
            resolved_path.display()
        ));
    }

    #[cfg(unix)]
    {
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!(
                "{} is not executable (missing execute permission bits)",
                resolved_path.display()
            ));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let extension = resolved_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();
        let has_executable_extension = matches!(extension.as_str(), "exe" | "com" | "bat" | "cmd");
        if !has_executable_extension {
            return Err(format!(
                "{} is not an executable file (expected .exe/.com/.bat/.cmd)",
                resolved_path.display()
            ));
        }
    }

    Ok(resolved_path)
}

fn resolve_command_path(candidate: &str) -> Option<PathBuf> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }

    if is_explicit_path(trimmed) {
        return Some(PathBuf::from(trimmed));
    }

    let path_var = std::env::var_os("PATH")?;

    #[cfg(target_os = "windows")]
    {
        let has_extension = Path::new(trimmed).extension().is_some();
        let extensions = std::env::var("PATHEXT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                if value.starts_with('.') {
                    value.to_string()
                } else {
                    format!(".{value}")
                }
            })
            .collect::<Vec<_>>();

        for dir in std::env::split_paths(&path_var) {
            if has_extension {
                let candidate_path = dir.join(trimmed);
                if candidate_path.exists() {
                    return Some(candidate_path);
                }
                continue;
            }

            for extension in &extensions {
                let candidate_path = dir.join(format!("{trimmed}{extension}"));
                if candidate_path.exists() {
                    return Some(candidate_path);
                }
            }
        }
        return None;
    }

    #[cfg(not(target_os = "windows"))]
    {
        for dir in std::env::split_paths(&path_var) {
            let candidate_path = dir.join(trimmed);
            if candidate_path.exists() {
                return Some(candidate_path);
            }
        }
    }

    None
}

fn is_explicit_path(value: &str) -> bool {
    Path::new(value).is_absolute() || value.contains('/') || value.contains('\\')
}

fn preferred_whisper_cli_names() -> Vec<String> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let mut names = Vec::<String>::new();

    if os == "windows" {
        names.push(format!("whisper-cli-{arch}-pc-windows-msvc.exe"));
        names.push("whisper-cli.exe".to_string());
    } else if os == "macos" {
        names.push(format!("whisper-cli-{arch}-apple-darwin"));
        names.push("whisper-cli".to_string());
    } else if os == "linux" {
        names.push(format!("whisper-cli-{arch}-unknown-linux-gnu"));
        names.push("whisper-cli".to_string());
    } else {
        names.push("whisper-cli".to_string());
    }

    names
}

fn find_whisper_cli_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in preferred_whisper_cli_names() {
        let preferred = dir.join(name);
        if preferred.is_file() {
            return Some(preferred);
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

    // In tauri:dev, sidecar binaries usually live in src-tauri/binaries.
    candidate_dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries"));

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

fn local_dev_sidecar_candidates() -> Vec<String> {
    let binaries_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    preferred_whisper_cli_names()
        .into_iter()
        .map(|name| binaries_dir.join(name).to_string_lossy().to_string())
        .collect()
}

fn candidate_whisper_cli_paths(configured_path: &str) -> Vec<String> {
    let mut candidates = Vec::<String>::new();

    if !configured_path.trim().is_empty() {
        candidates.push(configured_path.trim().to_string());
    }
    candidates.extend(local_dev_sidecar_candidates());
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

fn resolve_local_paths(base_data_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let app_dir = base_data_dir.join(APP_SETTINGS_DIR);
    let models_dir = app_dir.join(APP_MODELS_DIR);
    let settings_path = app_dir.join(APP_SETTINGS_FILE);

    fs::create_dir_all(&app_dir).map_err(|e| {
        format!(
            "Failed to create local app settings directory {}: {e}",
            app_dir.display()
        )
    })?;
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

    match serde_json::from_str::<LocalSettings>(&raw) {
        Ok(settings) => settings,
        Err(error) => {
            log::warn!(
                "load_local_settings: failed to parse LocalSettings from {}: {}",
                settings_path.display(),
                error
            );
            LocalSettings::default()
        }
    }
}

fn save_local_settings(settings_path: &Path, settings: &LocalSettings) -> Result<(), String> {
    let parent = settings_path.parent().ok_or_else(|| {
        format!(
            "Failed to determine settings directory for {}",
            settings_path.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|e| {
        format!(
            "Failed to create settings directory {}: {e}",
            parent.display()
        )
    })?;

    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize dictation settings: {e}"))?;

    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let target_name = settings_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dictation-settings.json");
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        target_name,
        std::process::id(),
        timestamp_nanos
    ));

    let mut temp_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|e| {
            format!(
                "Failed to create temp settings file {}: {e}",
                temp_path.display()
            )
        })?;
    temp_file.write_all(serialized.as_bytes()).map_err(|e| {
        format!(
            "Failed to write temp settings file {}: {e}",
            temp_path.display()
        )
    })?;
    temp_file.flush().map_err(|e| {
        format!(
            "Failed to flush temp settings file {}: {e}",
            temp_path.display()
        )
    })?;
    temp_file.sync_all().map_err(|e| {
        format!(
            "Failed to sync temp settings file {}: {e}",
            temp_path.display()
        )
    })?;
    drop(temp_file);

    fs::rename(&temp_path, settings_path).map_err(|e| {
        format!(
            "Failed to replace dictation settings file {} with temp file {}: {e}",
            settings_path.display(),
            temp_path.display()
        )
    })?;

    Ok(())
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
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8(output.stdout)
            .ok()?
            .trim_matches(|c| c == '\r' || c == '\n' || c == ' ')
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

fn pick_best_installed_model(
    models_dir: &Path,
    total_memory_gb: u64,
    exclude_model_id: Option<&str>,
) -> Option<(WhisperModelSpec, PathBuf)> {
    whisper_model_catalog()
        .iter()
        .copied()
        .filter(|spec| !exclude_model_id.is_some_and(|exclude| exclude == spec.id))
        .filter_map(|spec| {
            let path = model_path_for_spec(models_dir, spec);
            if path.exists() {
                Some((spec, path))
            } else {
                None
            }
        })
        .max_by(|(a, _), (b, _)| {
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
    .arg("-ng")
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
async fn delete_dictation_model(
    model: String,
    model_state: State<'_, LocalModelState>,
) -> Result<DictationModelDeletion, String> {
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
    let total_memory_gb = system_memory_gb();

    let delete_task =
        tauri::async_runtime::spawn_blocking(move || -> Result<DictationModelDeletion, String> {
            let target_path = model_path_for_spec(&models_dir, model_spec);
            if let Err(e) = fs::remove_file(&target_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!(
                        "Failed to delete model '{}' at {}: {e}",
                        model_spec.id,
                        target_path.display()
                    ));
                }
            }

            let target_path_string = target_path.to_string_lossy().to_string();
            let (selected_model_id, selected_model_path) = {
                let mut settings = settings
                    .lock()
                    .map_err(|_| "Failed to lock local model settings".to_string())?;

                let deleted_selected_model = settings.selected_model_id.as_deref()
                    == Some(model_spec.id)
                    || settings
                        .selected_model_path
                        .as_deref()
                        .is_some_and(|path| path == target_path_string);

                if deleted_selected_model {
                    if let Some((fallback_spec, fallback_path)) =
                        pick_best_installed_model(&models_dir, total_memory_gb, Some(model_spec.id))
                    {
                        settings.selected_model_id = Some(fallback_spec.id.to_string());
                        settings.selected_model_path =
                            Some(fallback_path.to_string_lossy().to_string());
                    } else {
                        settings.selected_model_id = None;
                        settings.selected_model_path = None;
                    }
                    save_local_settings(&settings_path, &settings)?;
                }

                (
                    settings.selected_model_id.clone(),
                    settings.selected_model_path.clone(),
                )
            };

            Ok(DictationModelDeletion {
                deleted_model_id: model_spec.id.to_string(),
                selected_model_id,
                selected_model_path,
            })
        });

    delete_task
        .await
        .map_err(|e| format!("Model delete task failed: {e}"))?
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

#[cfg(test)]
mod tests {
    use super::{resample_linear, whisper_help_text_looks_valid};

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
    fn whisper_help_text_accepts_real_help_snippet() {
        let stdout = "usage: whisper-cli [options] file0.wav\n  -m FNAME  model path";
        assert!(whisper_help_text_looks_valid(stdout, ""));
    }

    #[test]
    fn whisper_help_text_rejects_placeholder_snippet() {
        let stderr =
            "Bundled whisper-cli placeholder. Replace with a real whisper-cli sidecar binary.";
        assert!(!whisper_help_text_looks_valid("", stderr));
    }
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init();

    let whisper_model_path_override = std::env::var("WHISPER_MODEL_PATH").ok();
    let whisper_cli_path_override = std::env::var("WHISPER_CLI_PATH").ok();

    tauri::Builder::default()
        .setup(move |app| {
            let bundled_whisper_cli_path = resolve_bundled_whisper_cli_path(app.handle());
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                format!(
                    "Failed to resolve Tauri app data directory while initializing local dictation paths: {e}"
                )
            })?;
            let (models_dir, settings_path) = resolve_local_paths(&app_data_dir).map_err(|e| {
                format!(
                    "Failed to initialize local dictation model paths under {}: {e}",
                    app_data_dir.display()
                )
            })?;
            let initial_settings = load_local_settings(&settings_path);

            app.manage(AppConfig {
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
            get_dictation_onboarding,
            open_whisper_setup_page,
            install_dictation_model,
            delete_dictation_model,
            start_native_dictation,
            stop_native_dictation,
            cancel_native_dictation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
