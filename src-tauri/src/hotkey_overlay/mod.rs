//! Global hotkeys, overlay pill windows, menu bar tray, and background UI sync.
//!
//! Thin facade that re-exports deep submodules and owns dictation UI coordination.

mod background_ui;
#[cfg(target_os = "macos")]
mod macos_fn;
mod pill;
#[cfg(target_os = "macos")]
mod tray;
mod trigger;

pub(crate) use background_ui::{
    background_ui_preferences_payload, current_background_ui_preferences, should_start_hidden,
    show_main_window, sync_background_ui,
};
pub(crate) use pill::create_pill_overlay_windows;
#[cfg(target_os = "macos")]
pub(crate) use trigger::refresh_macos_fn_listener_after_activation;
pub(crate) use trigger::{
    apply_registered_hotkey, current_registered_hotkey, current_trigger_runtime_details,
    default_dictation_trigger, dictation_trigger_payload, focused_field_insert_enabled,
    normalize_dictation_trigger, onboarding_runtime_details, resolve_effective_dictation_trigger,
    GlobalHotkeyState,
};

use crate::dictation_session::{self, is_benign_session_error};
use crate::state::{
    BackendDictationStatus, BackendHotkeyAction, DictationStatePayload, DICTATION_STATE_EVENT,
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
    emit_dictation_state_with_request(app, state, error, transcript, session_id, None);
}

pub(crate) fn emit_dictation_started(
    app: &tauri::AppHandle,
    session_id: u64,
    request_id: Option<String>,
) {
    emit_dictation_state_with_request(app, "listening", None, None, Some(session_id), request_id);
}

pub(crate) fn emit_dictation_state_with_request(
    app: &tauri::AppHandle,
    state: &str,
    error: Option<String>,
    transcript: Option<String>,
    session_id: Option<u64>,
    request_id: Option<String>,
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
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || sync_background_ui(&handle));
    app.emit(
        DICTATION_STATE_EVENT,
        DictationStatePayload {
            request_id,
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
    enum Prepared {
        Start(dictation_session::StartReservation, Option<u64>),
        Stop(Option<crate::state::ActiveRecording>),
    }
    let preparation = (|| -> Result<Prepared, String> {
        let dictation = app.state::<crate::state::DictationState>();
        match action {
            BackendHotkeyAction::HoldStart => {
                let hold = dictation.hold_intent.press();
                Ok(Prepared::Start(
                    dictation_session::reserve_start(app, None)?,
                    Some(hold),
                ))
            }
            BackendHotkeyAction::HoldStop => {
                dictation.hold_intent.release();
                Ok(Prepared::Stop(dictation_session::prepare_stop(app)?))
            }
            BackendHotkeyAction::Toggle => {
                if dictation_session::has_session(app)? {
                    Ok(Prepared::Stop(dictation_session::prepare_stop(app)?))
                } else {
                    Ok(Prepared::Start(
                        dictation_session::reserve_start(app, None)?,
                        None,
                    ))
                }
            }
        }
    })();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = match preparation {
            Ok(Prepared::Start(reservation, hold)) => {
                let start_handle = handle.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    dictation_session::start_reserved(&start_handle, reservation, hold, None)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result.map(|_| ()))
            }
            Ok(Prepared::Stop(recording)) => {
                dictation_session::stop_prepared(handle.clone(), recording)
                    .await
                    .map(|_| ())
            }
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            if !is_benign_session_error(&error) {
                log::warn!("Global hotkey action failed: {error}");
                emit_dictation_state(&handle, "error", Some(error), None, None);
            }
        }
    });
}
