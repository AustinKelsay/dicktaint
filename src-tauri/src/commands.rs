//! Tauri command handlers for settings, models, insert, and dictation IPC.

use crate::audio::list_input_devices;
use crate::dictation_session;
use crate::hotkey_overlay::{
    GlobalHotkeyState,
    apply_registered_hotkey, background_ui_preferences_payload,
    current_trigger_runtime_details, dictation_trigger_payload, emit_dictation_state,
    focused_field_insert_enabled, normalize_dictation_trigger,
    resolve_effective_dictation_trigger, sync_background_ui,
};
use crate::insert::{focused_field_insert_permission_status, insert_text_into_focused_field_impl};
use crate::models::{
    download_whisper_model, find_whisper_model_spec, model_file_looks_installed, model_path_for_spec,
    pick_best_installed_model, save_local_settings, system_memory_gb, whisper_model_catalog,
};
use crate::onboarding::build_onboarding_payload;
use crate::state::{
    parse_close_action, parse_menu_bar_mode, parse_pill_visibility_mode, AppConfig,
    BackgroundUiPreferencesPayload, DictationModelDeletion, DictationModelSelection,
    DictationOnboardingPayload, DictationTriggerPayload, FocusedFieldInsertPayload,
    LocalModelState, LocalSettings, WHISPER_CPP_SETUP_URL, resolve_background_ui_preferences,
};
use crate::whisper_cli::{
    detect_whisper_cli_path, ensure_whisper_cli_available, resolve_whisper_cli_path,
};
use std::fs;
use std::process::Command;
use std::sync::Arc;
use tauri::State;



#[tauri::command]
pub(crate) fn open_whisper_setup_page() -> Result<(), String> {
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

#[tauri::command]
pub(crate) fn get_dictation_onboarding(
    app: tauri::AppHandle,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationOnboardingPayload, String> {
    let mut payload =
        build_onboarding_payload(config.inner(), model_state.inner(), hotkey_state.inner())?;
    match apply_registered_hotkey(
        &app,
        hotkey_state.inner(),
        payload.dictation_trigger.as_deref(),
    ) {
        Ok(runtime) => {
            payload.dictation_trigger_mode = runtime.mode.as_str().to_string();
            payload.dictation_trigger_status = runtime.status;
            payload.dictation_trigger_permission_hint = runtime.permission_hint;
        }
        Err(error) => {
            log::warn!("get_dictation_onboarding: failed to apply global hotkey: {error}");
        }
    }
    sync_background_ui(&app);
    Ok(payload)
}

#[tauri::command]
pub(crate) fn get_dictation_trigger(
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    let runtime = current_trigger_runtime_details(hotkey_state.inner())?;
    Ok(dictation_trigger_payload(&settings, runtime))
}

#[tauri::command]
pub(crate) fn set_dictation_trigger(
    app: tauri::AppHandle,
    trigger: String,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let normalized = normalize_dictation_trigger(&trigger)?;
    let (previous_trigger, previous_trigger_raw, previous_trigger_enabled) = {
        let settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        (
            resolve_effective_dictation_trigger(&settings),
            settings.dictation_trigger.clone(),
            settings.dictation_trigger_enabled,
        )
    };

    let runtime = apply_registered_hotkey(&app, hotkey_state.inner(), Some(&normalized))?;

    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    settings.dictation_trigger = Some(normalized.clone());
    settings.dictation_trigger_enabled = Some(true);
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.dictation_trigger = previous_trigger_raw;
        settings.dictation_trigger_enabled = previous_trigger_enabled;
        drop(settings);
        if let Err(restore_error) =
            apply_registered_hotkey(&app, hotkey_state.inner(), previous_trigger.as_deref())
        {
            log::warn!("set_dictation_trigger: failed to restore previous hotkey after save error: {restore_error}");
        }
        return Err(error);
    }
    sync_background_ui(&app);
    Ok(dictation_trigger_payload(&settings, runtime))
}

#[tauri::command]
pub(crate) fn clear_dictation_trigger(
    app: tauri::AppHandle,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let (previous_trigger, previous_trigger_raw, previous_trigger_enabled) = {
        let settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        (
            resolve_effective_dictation_trigger(&settings),
            settings.dictation_trigger.clone(),
            settings.dictation_trigger_enabled,
        )
    };

    let runtime = apply_registered_hotkey(&app, hotkey_state.inner(), None)?;

    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    settings.dictation_trigger = None;
    settings.dictation_trigger_enabled = Some(false);
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.dictation_trigger = previous_trigger_raw;
        settings.dictation_trigger_enabled = previous_trigger_enabled;
        drop(settings);
        if let Err(restore_error) =
            apply_registered_hotkey(&app, hotkey_state.inner(), previous_trigger.as_deref())
        {
            log::warn!(
                "clear_dictation_trigger: failed to restore previous hotkey after save error: {restore_error}"
            );
        }
        return Err(error);
    }
    sync_background_ui(&app);
    Ok(dictation_trigger_payload(&settings, runtime))
}

fn persist_background_ui_preferences_update<F>(
    app: &tauri::AppHandle,
    model_state: &LocalModelState,
    update: F,
) -> Result<BackgroundUiPreferencesPayload, String>
where
    F: FnOnce(&mut LocalSettings) -> Result<(), String>,
{
    let settings_path = model_state.settings_path.clone();
    let payload = {
        let mut settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        let previous = settings.clone();
        update(&mut settings)?;

        let preferences = resolve_background_ui_preferences(&settings);
        settings.pill_visibility_mode = Some(preferences.pill_visibility_mode.as_str().to_string());
        settings.menu_bar_mode = Some(preferences.menu_bar_mode.as_str().to_string());
        settings.close_action = Some(preferences.close_action.as_str().to_string());

        if let Err(error) = save_local_settings(&settings_path, &settings) {
            *settings = previous;
            return Err(error);
        }

        background_ui_preferences_payload(preferences)
    };

    sync_background_ui(app);
    Ok(payload)
}

#[tauri::command]
pub(crate) fn set_pill_visibility_mode(
    app: tauri::AppHandle,
    mode: String,
    model_state: State<'_, LocalModelState>,
) -> Result<BackgroundUiPreferencesPayload, String> {
    let normalized = parse_pill_visibility_mode(&mode)?;

    persist_background_ui_preferences_update(&app, model_state.inner(), |settings| {
        settings.pill_visibility_mode = Some(normalized.as_str().to_string());
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn set_menu_bar_mode(
    app: tauri::AppHandle,
    mode: String,
    model_state: State<'_, LocalModelState>,
) -> Result<BackgroundUiPreferencesPayload, String> {
    let normalized = parse_menu_bar_mode(&mode)?;

    persist_background_ui_preferences_update(&app, model_state.inner(), |settings| {
        settings.menu_bar_mode = Some(normalized.as_str().to_string());
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn set_close_action(
    app: tauri::AppHandle,
    action: String,
    model_state: State<'_, LocalModelState>,
) -> Result<BackgroundUiPreferencesPayload, String> {
    let normalized = parse_close_action(&action)?;

    persist_background_ui_preferences_update(&app, model_state.inner(), |settings| {
        settings.close_action = Some(normalized.as_str().to_string());
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn set_focused_field_insert_enabled(
    enabled: bool,
    model_state: State<'_, LocalModelState>,
) -> Result<FocusedFieldInsertPayload, String> {
    let permission = focused_field_insert_permission_status(enabled, enabled);
    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    let previous = settings.focused_field_insert_enabled;
    settings.focused_field_insert_enabled = Some(enabled);
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.focused_field_insert_enabled = previous;
        return Err(error);
    }
    Ok(FocusedFieldInsertPayload {
        enabled: focused_field_insert_enabled(&settings),
        permission_granted: permission.granted,
        permission_status: permission.status,
    })
}

#[tauri::command]
pub(crate) fn set_preferred_input_device(
    device_name: Option<String>,
    model_state: State<'_, LocalModelState>,
) -> Result<Option<String>, String> {
    let normalized = device_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let available_devices = list_input_devices();
    if let Some(name) = normalized.as_deref() {
        if !available_devices.iter().any(|device| device.name == name) {
            return Err(format!(
                "Microphone '{}' is not currently available on this machine.",
                name
            ));
        }
    }

    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    let previous = settings.preferred_input_device.clone();
    settings.preferred_input_device = normalized.clone();
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.preferred_input_device = previous;
        return Err(error);
    }

    Ok(settings.preferred_input_device.clone())
}

#[tauri::command]
pub(crate) fn insert_text_into_focused_field(
    state: State<'_, LocalModelState>,
    text: String,
) -> Result<(), String> {
    let focused_field_insert_enabled = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        focused_field_insert_enabled(&settings)
    };
    if !focused_field_insert_enabled {
        return Err(
            "Focused-field insertion is disabled in settings. Enable \"Dictate Into Focused Field\" to use this command."
                .to_string(),
        );
    }
    insert_text_into_focused_field_impl(&text)
}

#[tauri::command]
pub(crate) async fn install_dictation_model(
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
            if target_path.exists() && !model_file_looks_installed(&target_path, model_spec) {
                let _ = fs::remove_file(&target_path);
            }
            if !model_file_looks_installed(&target_path, model_spec) {
                download_whisper_model(model_spec, &target_path)?;
                if !model_file_looks_installed(&target_path, model_spec) {
                    let _ = fs::remove_file(&target_path);
                    return Err(format!(
                        "Model download completed but file is missing or too small at {}.",
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
pub(crate) async fn delete_dictation_model(
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
pub(crate) fn start_native_dictation(app: tauri::AppHandle) -> Result<(), String> {
    match dictation_session::start(&app) {
        Ok(_) => Ok(()),
        Err(error) => {
            if error.trim() != "Dictation already running." {
                emit_dictation_state(&app, "error", Some(error.clone()), None, None);
            }
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) async fn stop_native_dictation(app: tauri::AppHandle) -> Result<String, String> {
    dictation_session::stop(app).await
}

#[tauri::command]
pub(crate) fn cancel_native_dictation(app: tauri::AppHandle) -> Result<(), String> {
    dictation_session::cancel(&app)
}
