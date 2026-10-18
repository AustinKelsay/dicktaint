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
        (self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                Some((value | 1) + 2)
            })
            .unwrap()
            | 1)
            + 2
    }
    pub(crate) fn is_held(&self) -> bool {
        self.0.load(Ordering::SeqCst) & 1 == 1
    }
    pub(crate) fn release(&self) {
        self.0.fetch_and(!1, Ordering::SeqCst);
    }
    pub(crate) fn is_current(&self, token: u64) -> bool {
        self.0.load(Ordering::SeqCst) == token
    }
}

/// One pending capture reservation, created before dispatching any worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StartReservation {
    pub(crate) session_id: u64,
}
#[derive(Default)]
pub(crate) struct SessionIntent {
    sequence: u64,
    pending: Option<StartReservation>,
    pending_request_id: Option<String>,
}
impl SessionIntent {
    pub(crate) fn reserve(
        &mut self,
        request_id: Option<String>,
    ) -> Result<StartReservation, String> {
        if self.pending.is_some() {
            return Err("Dictation already running.".into());
        }
        self.sequence += 1;
        let reservation = StartReservation {
            session_id: self.sequence,
        };
        self.pending = Some(reservation);
        self.pending_request_id = request_id;
        Ok(reservation)
    }
    pub(crate) fn cancel(&mut self) -> Option<(u64, Option<String>)> {
        self.pending
            .take()
            .map(|value| (value.session_id, self.pending_request_id.take()))
    }
    pub(crate) fn is_current(&self, reservation: StartReservation) -> bool {
        self.pending == Some(reservation)
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(crate) fn finish(&mut self, reservation: StartReservation) {
        if self.is_current(reservation) {
            self.pending = None;
            self.pending_request_id = None;
        }
    }
}

pub(crate) fn reserve_start(
    app: &tauri::AppHandle,
    request_id: Option<String>,
) -> Result<StartReservation, String> {
    let dictation = app.state::<DictationState>();
    let mut intent = dictation
        .session_intent
        .lock()
        .map_err(|_| "Failed to lock session intent")?;
    if is_running(app)? {
        return Err("Dictation already running.".into());
    }
    intent.reserve(request_id)
}

pub(crate) fn has_session(app: &tauri::AppHandle) -> Result<bool, String> {
    let dictation = app.state::<DictationState>();
    let intent = dictation
        .session_intent
        .lock()
        .map_err(|_| "Failed to lock session intent")?;
    Ok(intent.has_pending() || is_running(app)?)
}

/// Returns true when `msg` is a benign concurrent start/stop conflict.
pub(crate) fn is_benign_session_error(msg: &str) -> bool {
    let trimmed = msg.trim();
    trimmed == "Dictation already running."
        || trimmed == "Dictation is not running."
        || trimmed == "Dictation start canceled."
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

/// Executes a reservation. Cancellation never waits for authorization or the start gate.
pub(crate) fn start_reserved(
    app: &tauri::AppHandle,
    reservation: StartReservation,
    hold: Option<u64>,
    request_id: Option<String>,
) -> Result<u64, String> {
    let mut result = start_reserved_inner(app, reservation, hold, request_id);
    if result.is_err() {
        let dictation = app.state::<DictationState>();
        if let Ok(mut intent) = dictation.session_intent.lock() {
            if !intent.is_current(reservation) {
                result = Err("Dictation start canceled.".into());
            }
            intent.finish(reservation);
        };
    }
    result
}

fn start_reserved_inner(
    app: &tauri::AppHandle,
    reservation: StartReservation,
    hold: Option<u64>,
    request_id: Option<String>,
) -> Result<u64, String> {
    let config = app.state::<AppConfig>();
    let model_state = app.state::<LocalModelState>();
    let dictation = app.state::<DictationState>();
    let _start_guard = dictation
        .start_gate
        .lock()
        .map_err(|_| "Failed to lock capture startup")?;
    if !dictation
        .session_intent
        .lock()
        .map_err(|_| "Failed to lock session intent")?
        .is_current(reservation)
        || hold.is_some_and(|token| !dictation.hold_intent.is_current(token))
    {
        return Err("Dictation start canceled.".into());
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

    if !dictation
        .session_intent
        .lock()
        .map_err(|_| "Failed to lock session intent")?
        .is_current(reservation)
    {
        return Err("Dictation start canceled.".into());
    }
    let session_id = reservation.session_id;
    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let (stop_tx, thread_handle, sample_rate, input_device_name) =
        spawn_recording_thread(Arc::clone(&samples), app.clone(), session_id)?;
    // Publication and cancellation share the short intent lock. Device opening and
    // authorization hold neither this lock nor active_recording.
    let mut intent = dictation
        .session_intent
        .lock()
        .map_err(|_| "Failed to lock session intent")?;
    if !intent.is_current(reservation)
        || hold.is_some_and(|token| !dictation.hold_intent.is_current(token))
    {
        drop(intent);
        let _ = stop_tx.send(());
        let _ = thread_handle.join();
        return Err("Dictation start canceled.".into());
    }
    *dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state")? = Some(ActiveRecording {
        start_request_id: request_id.clone(),
        session_id,
        input_device_name,
        stop_tx,
        thread_handle,
        samples,
        sample_rate,
    });
    intent.finish(reservation);
    crate::hotkey_overlay::emit_dictation_started(app, session_id, request_id);
    Ok(session_id)
}

/// Invalidates queued startup and takes capture before any deferred stop worker.
pub(crate) fn prepare_stop(app: &tauri::AppHandle) -> Result<Option<ActiveRecording>, String> {
    let dictation = app.state::<DictationState>();
    let mut intent = dictation
        .session_intent
        .lock()
        .map_err(|_| "Failed to lock session intent")?;
    let pending_id = intent.cancel();
    let recording = dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state")?
        .take();
    if recording.is_none() {
        if let Some((session_id, request_id)) = pending_id {
            crate::hotkey_overlay::emit_dictation_state_with_request(
                app,
                "idle",
                None,
                None,
                Some(session_id),
                request_id,
            );
        }
    }
    Ok(recording)
}

/// Stops capture, transcribes, and emits processing/idle/error states.
pub(crate) async fn stop(app: tauri::AppHandle) -> Result<String, String> {
    let recording = prepare_stop(&app)?;
    stop_prepared(app, recording).await
}

pub(crate) async fn stop_prepared(
    app: tauri::AppHandle,
    recording: Option<ActiveRecording>,
) -> Result<String, String> {
    let recording = recording.ok_or_else(|| "Dictation is not running.".to_string())?;
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
            emit_dictation_state(&app, "error", Some(message.clone()), None, Some(session_id));
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
    let recording = prepare_stop(app)?;
    let Some(recording) = recording else {
        return Ok(());
    };
    let session_id = Some(recording.session_id);
    let request_id = recording.start_request_id.clone();

    let _ = recording.stop_tx.send(());
    let _ = recording.thread_handle.join();

    crate::hotkey_overlay::emit_dictation_state_with_request(app, "idle", None, None, session_id, request_id);
    Ok(())
}

/// Cancels the active session when one is running; no-op otherwise.
pub(crate) fn cancel_if_active(app: &tauri::AppHandle) -> Result<(), String> {
    cancel(app)
}

#[cfg(test)]
mod tests {
    use super::is_benign_session_error;

    #[test]
    fn cancellation_invalidates_queued_start_without_cancelling_a_newer_reservation() {
        let mut intent = super::SessionIntent::default();
        let queued = intent.reserve(Some("start-a".into())).unwrap();
        assert_eq!(
            intent.cancel(),
            Some((queued.session_id, Some("start-a".into())))
        );
        assert!(!intent.is_current(queued));
        let next = intent.reserve(None).unwrap();
        intent.finish(queued);
        assert!(intent.is_current(next));
        assert!(intent.reserve(None).is_err());
        intent.finish(next);
        assert!(!intent.has_pending());
    }

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
