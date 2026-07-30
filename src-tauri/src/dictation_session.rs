//! Native dictation session orchestration: start, stop, cancel, and running-state queries.

use crate::audio::{ensure_microphone_access_authorized, spawn_recording_thread};
use crate::hotkey_overlay::emit_dictation_state;
use crate::models::resolve_active_model_path;
use crate::state::{ActiveRecording, AppConfig, DictationState, LocalModelState};
use crate::transcribe::transcribe_samples;
use crate::whisper_cli::{
    detect_whisper_cli_path, ensure_whisper_cli_available, resolve_whisper_cli_path,
};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::Manager;

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
    let config = app.state::<AppConfig>();
    let model_state = app.state::<LocalModelState>();
    let dictation = app.state::<DictationState>();

    ensure_microphone_access_authorized(app)?;
    resolve_active_model_path(config.inner(), model_state.inner())?;
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);
    ensure_whisper_cli_available(&whisper_cli_path)?;

    let mut guard = dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())?;
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

    let captured_samples = recording
        .samples
        .lock()
        .map_err(|_| "Failed to read captured audio".to_string())?
        .clone();
    let model_path = {
        let config = app.state::<AppConfig>();
        let model_state = app.state::<LocalModelState>();
        resolve_active_model_path(config.inner(), model_state.inner())?
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
    fn is_benign_session_error_matches_known_conflicts() {
        assert!(is_benign_session_error("Dictation already running."));
        assert!(is_benign_session_error("  Dictation is not running.  "));
        assert!(!is_benign_session_error("Audio capture thread crashed."));
    }
}
