/** Native macOS desktop dictation session control. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { NATIVE_HOLD_HOTKEYS, DICTATION_WAVEFORM_BAR_COUNT } from './constants.js';
import {
  getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage
} from './platform.js';
import {
  listeningStatusForTrigger, completedStatusForTrigger, eventKeyToken
} from './settings/hotkey-logic.js';
import {
  setUiMode, setStatus, setHotkeyPill, setAppScreen, setDictationState, syncControls
} from './ui.js';
import { appendTranscriptChunk, normalizeNativeSessionId } from './transcript.js';
import { ensureMicrophoneAccess } from './media-permissions.js';
import { clampAudioLevel, normalizeLiveAudioBars, updateDictationWaveform } from './waveform.js';

export function normalizeNativeDictationError(text) {
  return String(text || '').trim().toLowerCase();
}


export function isStartConflictDictationError(text) {
  const normalized = normalizeNativeDictationError(text);
  return normalized.includes('dictation already running');
}


export function isStopNoopDictationError(text) {
  const normalized = normalizeNativeDictationError(text);
  return normalized.includes('dictation is not running');
}


export function queueNativeStartAfterCurrentStop(trigger = 'hotkey') {
  state.pendingNativeStartAfterStop = true;
  state.pendingNativeStartTrigger = trigger;
}


export async function maybeStartQueuedNativeDictation() {
  if (!state.pendingNativeStartAfterStop) return;
  if (state.isDictating || state.isStartingDictation || state.nativeStopRequestInFlight) return;

  const trigger = state.pendingNativeStartTrigger || 'hotkey';
  state.pendingNativeStartAfterStop = false;
  state.pendingNativeStartTrigger = null;
  const startFn = state.startNativeDesktopDictationOverride || startNativeDesktopDictation;
  try {
    await startFn(trigger);
  } catch (error) {
    const details = getErrorMessage(error);
    state.isStartingDictation = false;
    setDictationState(false);
    setUiMode('error');
    setStatus(`Could not start dictation: ${details}`, 'error');
    console.error('Could not start queued dictation', error);
  }
}


export async function startNativeDesktopDictation(trigger = 'button', shouldRetryOnConflict = true) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;
  if (state.nativeStopRequestInFlight) {
    queueNativeStartAfterCurrentStop(trigger);
    setStatus('Finishing previous dictation... next one will start automatically.', 'working');
    return;
  }
  if (state.isDictating || state.isStartingDictation) return;

  if (!state.nativeDictationModelReady) {
    setStatus('Complete setup first, then start dictation.', 'neutral');
    return;
  }

  try {
    state.isStartingDictation = true;
    state.nativeSessionIdToIgnore = null;
    state.rejectNextNativeAppend = false;
    syncControls();
    setUiMode('loading');
    setStatus('Requesting microphone access...', 'working');
    await ensureMicrophoneAccess();
    setStatus('Opening microphone...', 'working');
    state.activeNativeSessionId = null;
    await tauriInvoke('start_native_dictation');
    state.isStartingDictation = false;
    setDictationState(true);
    setUiMode('listening');
    setStatus(listeningStatusForTrigger(trigger), 'live');
  } catch (error) {
    const details = getErrorMessage(error);
    if (shouldRetryOnConflict && isStartConflictDictationError(details)) {
      setStatus('Recovering from stale dictation state...', 'working');
      state.nativeSessionIdToIgnore = null;
      state.rejectNextNativeAppend = false;
      state.activeNativeSessionId = null;
      state.isStartingDictation = false;
      setDictationState(false);
      try {
        await tauriInvoke('cancel_native_dictation');
        return startNativeDesktopDictation(trigger, false);
      } catch (recoverError) {
        state.isStartingDictation = false;
        setDictationState(false);
        setUiMode('error');
        setStatus(`Could not recover dictation session: ${getErrorMessage(recoverError)}`, 'error');
        return;
      }
    }

    state.isStartingDictation = false;
    setDictationState(false);
    state.activeNativeSessionId = null;
    setUiMode('error');
    setStatus(`Could not start dictation: ${details}`, 'error');
  }
}


export async function stopNativeDesktopDictation(trigger = 'button') {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || (!state.isDictating && !state.isStartingDictation)) return;
  if (state.nativeStopRequestInFlight) return;

  state.nativeStopRequestInFlight = true;
  const sessionId = state.activeNativeSessionId;
  try {
    setUiMode('loading');
    setStatus('Transcribing captured audio...', 'working');
    const transcript = await tauriInvoke('stop_native_dictation');
    const didAppendTranscript = appendTranscriptChunk(transcript, {
      source: 'native',
      nativeSessionId: sessionId
    });
    setUiMode('idle');
    if (didAppendTranscript) {
      setStatus(completedStatusForTrigger(trigger), 'ok');
    } else {
      setStatus('No new dictation content to save.', 'neutral');
    }
  } catch (error) {
    const details = getErrorMessage(error);
    if (isStopNoopDictationError(details)) {
      setUiMode('idle');
      state.activeNativeSessionId = null;
      setStatus('No active dictation session to stop.', 'neutral');
      return;
    }
    setUiMode('error');
    setStatus(`Could not stop dictation: ${details}`, 'error');
  } finally {
    state.nativeStopRequestInFlight = false;
    state.isStartingDictation = false;
    setDictationState(false);
    void maybeStartQueuedNativeDictation();
  }
}


export function isNativeHoldHotkeyEvent(event) {
  const key = eventKeyToken(event);
  if (key && NATIVE_HOLD_HOTKEYS.has(key)) return true;

  const keyName = String(event?.key || '').trim().toLowerCase();
  if (keyName === 'fn' || keyName === 'f19') return true;
  return keyName === 'unidentified' && Boolean(event.getModifierState?.('Fn'));
}


export function applyNativeFnHoldState(pressed) {
  if (!isFocusedMacDesktopMode()) return;
  const nextPressed = Boolean(pressed);

  if (nextPressed) {
    state.nativeFnHoldActive = true;
    state.nativeFnStopRequested = false;
    if (state.nativeStopRequestInFlight) {
      queueNativeStartAfterCurrentStop('hotkey-hold');
      setHotkeyPill('finishing previous dictation...', 'working', true);
      return;
    }

    if (state.isDictating || state.isStartingDictation || state.nativeHotkeyActionInFlight) return;
    state.nativeHotkeyActionInFlight = true;
    setHotkeyPill('fn down - starting dictation...', 'working', true);
    Promise.resolve(startNativeDesktopDictation('hotkey-hold'))
      .catch(() => {})
      .finally(() => {
        state.nativeHotkeyActionInFlight = false;
        if (state.nativeFnStopRequested || !state.nativeFnHoldActive) {
          state.nativeFnStopRequested = false;
          applyNativeFnHoldState(false);
        }
      });
    return;
  }

  state.nativeFnHoldActive = false;
  if (state.isStartingDictation && !state.isDictating) {
    // Release can arrive while startup is in-flight; defer stop until capture is live.
    state.nativeFnStopRequested = true;
    setHotkeyPill('fn released - waiting for microphone...', 'working', true);
    return;
  }

  if (!state.isDictating || state.nativeHotkeyActionInFlight) return;
  state.nativeHotkeyActionInFlight = true;
  setHotkeyPill('fn released - transcribing...', 'working', true);
  Promise.resolve(stopNativeDesktopDictation('hotkey-hold'))
    .catch(() => {})
    .finally(() => {
      state.nativeHotkeyActionInFlight = false;
    });
}


export function handleNativeHoldKeydown(event) {
  if (!isFocusedMacDesktopMode()) return;
  if (state.dictationTriggerMode !== 'focused-window-hold') return;
  if (event.repeat) return;
  if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
  if (!isNativeHoldHotkeyEvent(event)) return;
  if (state.activeHotkeySpec?.ok && state.activeHotkeySpec.key !== 'Fn') return;

  event.preventDefault();
  event.stopPropagation();
  applyNativeFnHoldState(true);
}


export function handleNativeHoldKeyup(event) {
  if (!isFocusedMacDesktopMode()) return;
  if (state.dictationTriggerMode !== 'focused-window-hold') return;
  if (!isNativeHoldHotkeyEvent(event)) return;
  if (state.activeHotkeySpec?.ok && state.activeHotkeySpec.key !== 'Fn') return;

  event.preventDefault();
  event.stopPropagation();
  applyNativeFnHoldState(false);
}


export function triggerDictationToggleFromHotkey() {
  const now = Date.now();
  if (now - state.lastHotkeyToggleAtMs < 180) return;
  state.lastHotkeyToggleAtMs = now;

  if (state.isInstallingDictationModel || state.isDeletingDictationModel) return;
  if (!state.nativeDictationModelReady) {
    setStatus('Complete setup first, then start dictation.', 'neutral');
    return;
  }
  if (state.nativeStopRequestInFlight) {
    queueNativeStartAfterCurrentStop('hotkey');
    setStatus('Finishing previous dictation... next one will start automatically.', 'working');
    return;
  }

  if (state.isDictating || state.isStartingDictation) {
    if (!dom.stopDictationBtn.disabled) {
      dom.stopDictationBtn.click();
    }
    return;
  }

  setAppScreen('dictation');
  if (!dom.startDictationBtn.disabled) {
    dom.startDictationBtn.click();
  }
}


export function handleNativeDictationStatePayload(payload) {
  const s = payload?.state ?? 'idle';
  const payloadSessionId = normalizeNativeSessionId(payload?.session_id);
  const sessionMatchesCurrent = !payloadSessionId
    || !state.activeNativeSessionId
    || payloadSessionId === state.activeNativeSessionId;

  if (s === 'listening') {
    // Ignore late listening events from a superseded session; allow first
    // listening when activeNativeSessionId is still null (sessionMatchesCurrent).
    if (!sessionMatchesCurrent) return;
    state.activeNativeSessionId = payloadSessionId || state.activeNativeSessionId;
    state.isStartingDictation = false;
    setDictationState(true);
    setUiMode('listening');
    setStatus('Listening\u2026 click Stop to transcribe.', 'live');
    return;
  }

  if (s === 'processing') {
    if (payloadSessionId && !state.activeNativeSessionId) {
      state.activeNativeSessionId = payloadSessionId;
    }
    state.isStartingDictation = false;
    setUiMode('loading');
    setStatus('Transcribing captured audio...', 'working');
    return;
  }

  if (s === 'idle') {
    const transcriptSessionId = payloadSessionId || state.activeNativeSessionId;
    const didAppendTranscript = state.nativeStopRequestInFlight
      ? false
      : appendTranscriptChunk(payload?.transcript, {
        source: 'native-event',
        nativeSessionId: transcriptSessionId
      });

    if (sessionMatchesCurrent && (state.isDictating || state.isStartingDictation || didAppendTranscript)) {
      state.isStartingDictation = false;
      setDictationState(false);
      setUiMode('idle');
    }
    if (sessionMatchesCurrent) {
      state.activeNativeSessionId = null;
    }
    if (didAppendTranscript && sessionMatchesCurrent) {
      setStatus('Dictation captured and transcribed.', 'ok');
    }
    return;
  }

  if (s === 'error') {
    const details = getErrorMessage(payload?.error);
    if (sessionMatchesCurrent) {
      state.activeNativeSessionId = null;
      state.nativeStopRequestInFlight = false;
      state.isStartingDictation = false;
      setDictationState(false);
      setUiMode('error');
      setStatus(`Could not transcribe dictation: ${details}`, 'error');
      void maybeStartQueuedNativeDictation();
    }
  }
}


export function handleNativeDictationAudioLevelPayload(payload) {
  const payloadSessionId = normalizeNativeSessionId(payload?.session_id);
  if (payloadSessionId && state.activeNativeSessionId && payloadSessionId !== state.activeNativeSessionId) {
    return;
  }

  const level = clampAudioLevel(payload?.level);
  const bars = normalizeLiveAudioBars(payload?.bars, level, DICTATION_WAVEFORM_BAR_COUNT);
  updateDictationWaveform(level, bars, 'listening');
}

