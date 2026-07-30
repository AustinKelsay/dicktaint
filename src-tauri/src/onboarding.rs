//! Assembles the dictation onboarding DTO from deep modules.
//!
//! Keeps catalog/settings/path logic in `models` while composing hotkey, insert,
//! audio, and whisper-cli readiness into one payload for the frontend gate.

use crate::audio::list_input_devices;
use crate::hotkey_overlay::{
    GlobalHotkeyState, current_registered_hotkey, current_trigger_runtime_details,
    default_dictation_trigger, focused_field_insert_enabled, onboarding_runtime_details,
    resolve_effective_dictation_trigger,
};
use crate::insert::focused_field_insert_permission_status;
use crate::models::{build_device_profile, build_model_options};
use crate::state::{
    resolve_background_ui_preferences, AppConfig, DictationOnboardingPayload, LocalModelState,
};
use crate::whisper_cli::{detect_whisper_cli_path, resolve_whisper_cli_path};
use std::path::Path;

/// Builds the onboarding payload used by the desktop readiness gate.
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
