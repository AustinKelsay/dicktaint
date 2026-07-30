//! Background UI preferences, backend status helpers, and window/tray visibility.

use crate::state::{
    BackendDictationStatus, BackgroundUiPreferences, BackgroundUiPreferencesPayload, DictationState,
    LocalModelState, MenuBarMode, START_HIDDEN_ENV, resolve_background_ui_preferences,
};
use tauri::Manager;

use super::pill::sync_pill_for_backend_state;
#[cfg(target_os = "macos")]
use super::tray::sync_macos_tray;

fn parse_truthy_env(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    !matches!(normalized.as_str(), "" | "0" | "false" | "no" | "off")
}

pub(crate) fn should_start_hidden() -> bool {
    std::env::var(START_HIDDEN_ENV)
        .map(|value| parse_truthy_env(&value))
        .unwrap_or(false)
}

pub(crate) fn background_ui_preferences_payload(
    preferences: BackgroundUiPreferences,
) -> BackgroundUiPreferencesPayload {
    BackgroundUiPreferencesPayload {
        pill_visibility_mode: preferences.pill_visibility_mode.as_str().to_string(),
        menu_bar_mode: preferences.menu_bar_mode.as_str().to_string(),
        close_action: preferences.close_action.as_str().to_string(),
    }
}

pub(crate) fn current_background_ui_preferences(
    app: &tauri::AppHandle,
) -> Result<BackgroundUiPreferences, String> {
    let settings = app
        .state::<LocalModelState>()
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    Ok(resolve_background_ui_preferences(&settings))
}

pub(super) fn current_backend_dictation_status(
    app: &tauri::AppHandle,
) -> Result<BackendDictationStatus, String> {
    app.state::<DictationState>()
        .backend_status
        .lock()
        .map_err(|_| "Failed to lock backend dictation status".to_string())
        .map(|guard| *guard)
}

fn current_backend_error_message(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    app.state::<DictationState>()
        .last_error_message
        .lock()
        .map_err(|_| "Failed to lock backend dictation error state".to_string())
        .map(|guard| guard.clone())
}

pub(super) fn set_backend_dictation_status(
    app: &tauri::AppHandle,
    status: BackendDictationStatus,
    error: Option<String>,
) -> Result<(), String> {
    let dictation = app.state::<DictationState>();
    {
        let mut guard = dictation
            .backend_status
            .lock()
            .map_err(|_| "Failed to lock backend dictation status".to_string())?;
        *guard = status;
    }
    let mut error_guard = dictation
        .last_error_message
        .lock()
        .map_err(|_| "Failed to lock backend dictation error state".to_string())?;
    *error_guard = if status == BackendDictationStatus::Error {
        error.filter(|value| !value.trim().is_empty())
    } else {
        None
    };
    Ok(())
}

pub(super) fn main_window_is_visible(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

pub(super) fn should_show_tray_icon(preferences: BackgroundUiPreferences, main_window_visible: bool) -> bool {
    match preferences.menu_bar_mode {
        MenuBarMode::Always => true,
        MenuBarMode::BackgroundOnly => !main_window_visible,
        MenuBarMode::Off => false,
    }
}

pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    sync_background_ui(app);
}

pub(crate) fn sync_background_ui(app: &tauri::AppHandle) {
    let status = current_backend_dictation_status(app).unwrap_or_default();
    let error = current_backend_error_message(app).ok().flatten();
    sync_pill_for_backend_state(app, status, error.as_deref());
    #[cfg(target_os = "macos")]
    if let Err(error) = sync_macos_tray(app, status) {
        log::warn!("Failed to sync macOS tray state: {error}");
    }
}
