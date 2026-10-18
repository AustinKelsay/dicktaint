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
    should_start_hidden, show_main_window, sync_background_ui,
};
pub(crate) use pill::create_pill_overlay_windows;
pub(crate) use trigger::{
    apply_registered_hotkey, current_registered_hotkey, current_trigger_runtime_details,
    default_dictation_trigger, dictation_trigger_payload, focused_field_insert_enabled,
    normalize_dictation_trigger, onboarding_runtime_details, resolve_effective_dictation_trigger,
    GlobalHotkeyState,
};
#[cfg(target_os = "macos")]
pub(crate) use trigger::refresh_macos_fn_listener_after_activation;

use crate::dictation_session::{is_benign_session_error, is_running, start, stop};
use crate::state::{
    BackendDictationStatus, BackendHotkeyAction, DICTATION_STATE_EVENT, DictationStatePayload,
};
use tauri::{Emitter, Manager};

use background_ui::set_backend_dictation_status;

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
    // Capture press/release synchronously, before async tasks can be reordered.
    let dictation = app.state::<crate::state::DictationState>();
    let hold_token = match action {
        BackendHotkeyAction::HoldStart => Some(dictation.hold_intent.press()),
        BackendHotkeyAction::HoldStop => { dictation.hold_intent.release(); None },
        _ => None,
    };
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result: Result<(), String> = match action {
            BackendHotkeyAction::Toggle => match is_running(&handle) {
                Ok(true) => stop(handle.clone()).await.map(|_| ()),
                Ok(false) => {
                    let start_handle = handle.clone();
                    tauri::async_runtime::spawn_blocking(move || start(&start_handle)).await
                        .map_err(|e| e.to_string()).and_then(|result| result.map(|_| ()))
                },
                Err(error) => Err(error),
            },
            BackendHotkeyAction::HoldStart => match is_running(&handle) {
                Ok(true) => Ok(()),
                Ok(false) => {
                    let start_handle = handle.clone();
                    tauri::async_runtime::spawn_blocking(move || crate::dictation_session::start_with_hold(&start_handle, hold_token)).await
                        .map_err(|e| e.to_string()).and_then(|result| result.map(|_| ()))
                },
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
