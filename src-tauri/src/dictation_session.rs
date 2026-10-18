//! Native dictation session orchestration: start, stop, cancel, and running-state queries.

use crate::audio::{ensure_microphone_access_authorized, spawn_recording_thread};
use crate::hotkey_overlay::emit_dictation_state;
use crate::models::resolve_active_model_path;
use crate::state::{ActiveRecording, AppConfig, DictationState, LocalModelState};
use crate::transcribe::transcribe_samples;
use crate::whisper_cli::{
    detect_whisper_cli_path, ensure_whisper_cli_available, resolve_whisper_cli_path,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Generation token preserves hold/release intent across asynchronous permission prompts.
#[derive(Default)]
pub(crate) struct HoldIntent(AtomicU64);
impl HoldIntent {
    pub(crate) fn press(&self) -> u64 {
        (self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| Some((value | 1) + 2)).unwrap() | 1) + 2
    }
    pub(crate) fn is_held(&self) -> bool { self.0.load(Ordering::SeqCst) & 1 == 1 }
    pub(crate) fn release(&self) { self.0.fetch_and(!1, Ordering::SeqCst); }
    pub(crate) fn is_current(&self, token: u64) -> bool { self.0.load(Ordering::SeqCst) == token }
}

/// Returns true when `msg` is a benign concurrent start/stop conflict.
pub(crate) fn is_benign_session_error(msg: &str) -> bool {
    let trimmed = msg.trim();
    trimmed == "Dictation already running." || trimmed == "Dictation is not running."
}

/// Returns the active recording session id, if any.
pub(crate) fn current_active_session_id(app: &tauri::AppHandle) -> Result<Option<u64>, String> {
    let dictation = app.state::<DictationState>();
    dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())
        .map(|guard| guard.as_ref().map(|recording| recording.session_id))
}

/// Returns whether a native dictation session is currently recording.
pub(crate) fn is_running(app: &tauri::AppHandle) -> Result<bool, String> {
    current_active_session_id(app).map(|value| value.is_some())
}

/// Starts native mic capture and marks the session as listening.
pub(crate) fn start(app: &tauri::AppHandle) -> Result<u64, String> {
    start_with_hold(app, None)
}

pub(crate) fn start_with_hold(app: &tauri::AppHandle, hold: Option<u64>) -> Result<u64, String> {
    let config = app.state::<AppConfig>();
    let model_state = app.state::<LocalModelState>();
    let dictation = app.state::<DictationState>();

    // Refresh holds this same gate while replacing listeners. A pending permission
    // prompt is therefore protected just like an established recording.
    let _start_guard = dictation.start_gate.try_lock().map_err(|_| "Dictation already running.".to_string())?;
    if hold.is_some_and(|token| !dictation.hold_intent.is_current(token)) {
        return Err("Dictation is not running.".into());
    }
    resolve_active_model_path(config.inner(), model_state.inner())?;
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);
    ensure_whisper_cli_available(&whisper_cli_path)?;
    ensure_microphone_access_authorized(app)?;

    let mut guard = dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())?;
    if hold.is_some_and(|token| !dictation.hold_intent.is_current(token)) {
        return Err("Dictation is not running.".into());
    }
    if guard.is_some() {
        return Err("Dictation already running.".to_string());
    }

    let session_id = dictation.next_session_id.fetch_add(1, Ordering::SeqCst);
    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let (stop_tx, thread_handle, sample_rate, input_device_name) =
        spawn_recording_thread(Arc::clone(&samples), app.clone(), session_id)?;
    *guard = Some(ActiveRecording {
        session_id,
        input_device_name,
        stop_tx,
        thread_handle,
        samples,
        sample_rate,
    });
    drop(guard);

    emit_dictation_state(app, "listening", None, None, Some(session_id));
    Ok(session_id)
}

/// Stops capture, transcribes, and emits processing/idle/error states.
pub(crate) async fn stop(app: tauri::AppHandle) -> Result<String, String> {
    let recording = {
        let dictation = app.state::<DictationState>();
        let mut guard = dictation
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard
            .take()
            .ok_or_else(|| "Dictation is not running.".to_string())?
    };
    let session_id = recording.session_id;

    let _ = recording.stop_tx.send(());
    if recording.thread_handle.join().is_err() {
        emit_dictation_state(
            &app,
            "error",
            Some("Audio capture thread crashed.".into()),
            None,
            Some(session_id),
        );
        return Err("Audio capture thread crashed.".to_string());
    }

    let captured_samples = match recording.samples.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => {
            let message = "Failed to read captured audio".to_string();
            emit_dictation_state(
                &app,
                "error",
                Some(message.clone()),
                None,
                Some(session_id),
            );
            return Err(message);
        }
    };
    let model_path = {
        let config = app.state::<AppConfig>();
        let model_state = app.state::<LocalModelState>();
        match resolve_active_model_path(config.inner(), model_state.inner()) {
            Ok(path) => path,
            Err(error) => {
                emit_dictation_state(&app, "error", Some(error.clone()), None, Some(session_id));
                return Err(error);
            }
        }
    };
    let configured_whisper_cli_path = {
        let config = app.state::<AppConfig>();
        resolve_whisper_cli_path(
            config.whisper_cli_path_override.as_deref(),
            config.bundled_whisper_cli_path.as_deref(),
        )
    };
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);

    emit_dictation_state(&app, "processing", None, None, Some(session_id));

    let result = tauri::async_runtime::spawn_blocking(move || {
        transcribe_samples(
            model_path,
            whisper_cli_path,
            captured_samples,
            recording.sample_rate,
            recording.input_device_name,
        )
    })
    .await
    .map_err(|e| {
        emit_dictation_state(&app, "error", Some(e.to_string()), None, Some(session_id));
        format!("Failed to run transcription task: {e}")
    })?;

    match result {
        Ok(transcript) => {
            emit_dictation_state(
                &app,
                "idle",
                None,
                Some(transcript.clone()),
                Some(session_id),
            );
            Ok(transcript)
        }
        Err(e) => {
            emit_dictation_state(&app, "error", Some(e.clone()), None, Some(session_id));
            Err(e)
        }
    }
}

/// Cancels any active recording without transcription and returns to idle.
pub(crate) fn cancel(app: &tauri::AppHandle) -> Result<(), String> {
    let recording = {
        let dictation = app.state::<DictationState>();
        let mut guard = dictation
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard.take()
    };
    let session_id = recording.as_ref().map(|value| value.session_id);

    if let Some(recording) = recording {
        let _ = recording.stop_tx.send(());
        let _ = recording.thread_handle.join();
    }

    emit_dictation_state(app, "idle", None, None, session_id);
    Ok(())
}

/// Cancels the active session when one is running; no-op otherwise.
pub(crate) fn cancel_if_active(app: &tauri::AppHandle) -> Result<(), String> {
    if is_running(app)? {
        cancel(app)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_benign_session_error;

    #[test]
    fn released_hold_cannot_start_after_permission_prompt_or_new_press() {
        let intent = super::HoldIntent::default();
        let first = intent.press();
        assert!(intent.is_current(first));
        intent.release();
        assert!(!intent.is_current(first));
        let second = intent.press();
        assert!(!intent.is_current(first));
        assert!(intent.is_current(second));
    }

    #[test]
    fn is_benign_session_error_matches_known_conflicts() {
        assert!(is_benign_session_error("Dictation already running."));
        assert!(is_benign_session_error("  Dictation is not running.  "));
        assert!(!is_benign_session_error("Audio capture thread crashed."));
    }
}
