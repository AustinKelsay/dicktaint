//! Global hotkeys, overlay pill windows, menu bar tray, and background UI sync.
//!
//! Thin facade that re-exports deep submodules and owns dictation UI coordination.

mod background_ui;
#[cfg(target_os = "macos")]
mod macos_fn;
mod pill;
mod trigger;
#[cfg(target_os = "macos")]
mod tray;

pub(crate) use background_ui::{
    background_ui_preferences_payload, current_background_ui_preferences,
    set_backend_dictation_status, should_start_hidden, show_main_window, sync_background_ui,
};
pub(crate) use pill::create_pill_overlay_windows;
pub(crate) use trigger::{
    apply_registered_hotkey, current_registered_hotkey, current_trigger_runtime_details,
    default_dictation_trigger, dictation_trigger_payload, focused_field_insert_enabled,
    normalize_dictation_trigger, onboarding_runtime_details, resolve_effective_dictation_trigger,
    GlobalHotkeyState,
};

#[cfg(all(test, target_os = "macos"))]
pub(crate) use macos_fn::{
    macos_listener_disable_should_dispatch_stop, macos_tap_disable_should_dispatch_stop,
};
#[cfg(test)]
pub(crate) use pill::pill_should_be_visible_for_backend_state;
#[cfg(test)]
pub(crate) use trigger::{runtime_details_for_trigger, HotkeyDeliveryMode};
#[cfg(all(test, target_os = "macos"))]
pub(crate) use tray::{
    tray_force_stop_enabled, tray_primary_action_enabled, tray_primary_action_label,
};

use crate::dictation_session::{is_benign_session_error, is_running, start, stop};
use crate::state::{
    BackendDictationStatus, BackendHotkeyAction, DICTATION_STATE_EVENT, DictationStatePayload,
};
use tauri::Emitter;

/// Persists backend status, syncs overlay/tray UI, and emits the dictation state event.
pub(crate) fn emit_dictation_state(
    app: &tauri::AppHandle,
    state: &str,
    error: Option<String>,
    transcript: Option<String>,
    session_id: Option<u64>,
) {
    let backend_status = match state {
        "listening" => BackendDictationStatus::Listening,
        "processing" => BackendDictationStatus::Processing,
        "error" => BackendDictationStatus::Error,
        _ => BackendDictationStatus::Idle,
    };
    if let Err(status_error) = set_backend_dictation_status(app, backend_status, error.clone()) {
        log::warn!("Failed to update backend dictation status: {status_error}");
    }
    sync_background_ui(app);
    app.emit(
        DICTATION_STATE_EVENT,
        DictationStatePayload {
            state: state.to_string(),
            error,
            transcript,
            session_id,
        },
    )
    .ok();
}

/// Dispatches a backend hotkey action onto the async runtime.
pub(crate) fn dispatch_backend_hotkey_action(app: &tauri::AppHandle, action: BackendHotkeyAction) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result: Result<(), String> = match action {
            BackendHotkeyAction::Toggle => match is_running(&handle) {
                Ok(true) => stop(handle.clone()).await.map(|_| ()),
                Ok(false) => start(&handle).map(|_| ()),
                Err(error) => Err(error),
            },
            BackendHotkeyAction::HoldStart => match is_running(&handle) {
                Ok(true) => Ok(()),
                Ok(false) => start(&handle).map(|_| ()),
                Err(error) => Err(error),
            },
            BackendHotkeyAction::HoldStop => match is_running(&handle) {
                Ok(true) => stop(handle.clone()).await.map(|_| ()),
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            },
        };

        if let Err(error) = result {
            if !is_benign_session_error(&error) {
                log::warn!("Global hotkey action failed: {error}");
                emit_dictation_state(&handle, "error", Some(error), None, None);
            }
        }
    });
}
