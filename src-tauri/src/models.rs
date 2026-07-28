//! Whisper model catalog, local settings persistence, and onboarding payloads.

use crate::audio::list_input_devices;
use crate::hotkey_overlay::{
    GlobalHotkeyState,
    current_registered_hotkey, current_trigger_runtime_details, default_dictation_trigger,
    focused_field_insert_enabled, onboarding_runtime_details, resolve_background_ui_preferences,
    resolve_effective_dictation_trigger,
};
use crate::insert::focused_field_insert_permission_status;
use crate::state::{
    AppConfig, DeviceProfile, DictationInputDevice, DictationModelOption,
    DictationOnboardingPayload, LocalModelState, LocalSettings,
    WhisperModelSpec, WHISPER_MODEL_CATALOG, APP_MODELS_DIR, APP_SETTINGS_DIR,
    APP_SETTINGS_FILE,
};
use crate::whisper_cli::{detect_whisper_cli_path, resolve_whisper_cli_path};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};


pub(crate) fn resolve_whisper_model_path(path: Option<&str>) -> Result<PathBuf, String> {
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

pub(crate) fn resolve_local_paths(base_data_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
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

pub(crate) fn load_local_settings(settings_path: &Path) -> LocalSettings {
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

pub(crate) fn save_local_settings(settings_path: &Path, settings: &LocalSettings) -> Result<(), String> {
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
    // Write-then-rename keeps settings updates atomic across crashes/interruption.
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

pub(crate) fn whisper_model_catalog() -> &'static [WhisperModelSpec] {
    &WHISPER_MODEL_CATALOG
}

pub(crate) fn find_whisper_model_spec(id: &str) -> Option<WhisperModelSpec> {
    whisper_model_catalog()
        .iter()
        .copied()
        .find(|spec| spec.id == id)
}

pub(crate) fn total_memory_bytes() -> Option<u64> {
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

pub(crate) fn system_memory_gb() -> u64 {
    let total_bytes = total_memory_bytes().unwrap_or(8 * 1_073_741_824);
    (((total_bytes as f64) / 1_073_741_824.0).round() as u64).max(1)
}

pub(crate) fn build_device_profile() -> DeviceProfile {
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

pub(crate) fn model_path_for_spec(models_dir: &Path, spec: WhisperModelSpec) -> PathBuf {
    models_dir.join(spec.file_name)
}

pub(crate) fn model_fit_level(spec: WhisperModelSpec, total_memory_gb: u64) -> u8 {
    if total_memory_gb >= spec.recommended_ram_gb {
        2
    } else if total_memory_gb >= spec.min_ram_gb {
        1
    } else {
        0
    }
}

pub(crate) fn pick_recommended_model_id(total_memory_gb: u64) -> Option<&'static str> {
    whisper_model_catalog()
        .iter()
        .copied()
        .filter(|spec| model_fit_level(*spec, total_memory_gb) > 0)
        .max_by(|a, b| {
            // Prefer strongest runnable model for the machine, not merely the smallest.
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

pub(crate) fn build_model_options(
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

pub(crate) fn pick_best_installed_model(
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

pub(crate) fn resolve_active_model_path(
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

pub(crate) fn download_whisper_model(model_spec: WhisperModelSpec, target_path: &Path) -> Result<(), String> {
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

pub(crate) fn build_onboarding_payload(
    config: &AppConfig,
    model_state: &LocalModelState,
    hotkey_state: &GlobalHotkeyState,
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
    let dictation_trigger = resolve_effective_dictation_trigger(&settings);
    let registered_trigger = current_registered_hotkey(hotkey_state).ok().flatten();
    let registered_runtime = current_trigger_runtime_details(hotkey_state).ok();
    let trigger_runtime = onboarding_runtime_details(
        dictation_trigger.as_deref(),
        registered_trigger.as_deref(),
        registered_runtime.as_ref(),
    );
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
    let focused_field_permission =
        focused_field_insert_permission_status(focused_field_insert_enabled(&settings), false);
    let background_ui_preferences = resolve_background_ui_preferences(&settings);
    let available_input_devices = list_input_devices();

    Ok(DictationOnboardingPayload {
        onboarding_required,
        selected_model_id,
        selected_model_path,
        selected_model_exists,
        available_input_devices,
        preferred_input_device: settings.preferred_input_device.clone(),
        dictation_trigger,
        default_dictation_trigger: default_dictation_trigger(),
        dictation_trigger_mode: trigger_runtime.mode.as_str().to_string(),
        dictation_trigger_status: trigger_runtime.status,
        dictation_trigger_permission_hint: trigger_runtime.permission_hint,
        pill_visibility_mode: background_ui_preferences
            .pill_visibility_mode
            .as_str()
            .to_string(),
        menu_bar_mode: background_ui_preferences.menu_bar_mode.as_str().to_string(),
        close_action: background_ui_preferences.close_action.as_str().to_string(),
        focused_field_insert_enabled: focused_field_insert_enabled(&settings),
        focused_field_insert_permission_granted: focused_field_permission.granted,
        focused_field_insert_permission_status: focused_field_permission.status,
        whisper_cli_available,
        whisper_cli_path: detected_whisper_cli_path.unwrap_or(configured_whisper_cli_path),
        models_dir: model_state.models_dir.to_string_lossy().to_string(),
        device,
        models,
    })
}
