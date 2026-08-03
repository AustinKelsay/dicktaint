/**
 * Dictation hotkey capture and persistence.
 *
 * Pure parse/label helpers live in hotkey-logic.js (cycle-free leaf).
 */
import { state } from '../state.js';
import { DEFAULT_DICTATION_HOTKEY } from '../constants.js';
import {
  getTauriInvoke, isNativeDesktopMode, getErrorMessage
} from '../platform.js';
import { setStatus, syncControls } from '../ui.js';
import {
  setDictationHotkeyStatus,
  parseHotkeyCombo,
  buildHotkeyFromEvent
} from './hotkey-logic.js';

export {
  setDictationHotkeyStatus,
  normalizeHotkeyModifier,
  normalizeHotkeyKey,
  parseHotkeyCombo,
  eventKeyToken,
  buildHotkeyFromEvent,
  eventMatchesHotkey,
  getSuggestedHotkeyOptions,
  humanizeArchitecture,
  describeMachineLabel,
  instructionHotkeyLabel,
  isHoldToTalkHotkey,
  idlePillMessage,
  listeningStatusForTrigger,
  completedStatusForTrigger
} from './hotkey-logic.js';

/**
 * Applies hotkey fields from an onboarding/settings payload into state + UI.
 * @param {object} payload
 * @param {{ preservePending?: boolean }} [options]
 */
export function applyDictationHotkeyPayload(payload, { preservePending = false } = {}) {
  state.dictationTriggerMode = String(
    payload?.dictation_trigger_mode
      || payload?.trigger_mode
      || 'disabled'
  ).trim() || 'disabled';
  state.dictationTriggerStatus = String(
    payload?.dictation_trigger_status
      || payload?.trigger_status
      || 'Hotkey disabled.'
  ).trim() || 'Hotkey disabled.';
  state.dictationTriggerPermissionHint = String(
    payload?.dictation_trigger_permission_hint
      || payload?.trigger_permission_hint
      || ''
  ).trim();

  const rawDefault = String(
    payload?.default_trigger
      || payload?.default_dictation_trigger
      || DEFAULT_DICTATION_HOTKEY
  ).trim();
  const parsedDefault = parseHotkeyCombo(rawDefault);
  state.defaultDictationHotkey = parsedDefault.ok ? parsedDefault.display : DEFAULT_DICTATION_HOTKEY;

  const rawTrigger = String(payload?.trigger || payload?.dictation_trigger || '').trim();
  if (!rawTrigger) {
    state.savedDictationHotkey = null;
    state.activeHotkeySpec = null;
    if (!preservePending) state.pendingDictationHotkey = '';
    setDictationHotkeyStatus(`Hotkey disabled. Default: ${state.defaultDictationHotkey}.`, 'neutral');
    syncControls();
    return;
  }

  const parsedTrigger = parseHotkeyCombo(rawTrigger);
  if (!parsedTrigger.ok) {
    state.savedDictationHotkey = null;
    state.activeHotkeySpec = null;
    if (!preservePending) state.pendingDictationHotkey = '';
    setDictationHotkeyStatus(`Saved hotkey ignored: ${parsedTrigger.error}`, 'error');
    syncControls();
    return;
  }

  state.savedDictationHotkey = parsedTrigger.display;
  state.activeHotkeySpec = parsedTrigger;
  if (!preservePending) state.pendingDictationHotkey = parsedTrigger.display;
  setDictationHotkeyStatus(`Current hotkey: ${parsedTrigger.display}. ${state.dictationTriggerStatus}`, 'ok');
  syncControls();
}

/** Starts keyboard capture for a new hotkey combo. */
export function beginDictationHotkeyCapture() {
  if (!isNativeDesktopMode()) return;
  state.isCapturingDictationHotkey = true;
  setDictationHotkeyStatus('Press your desired key combo now. Press Escape to cancel. For Fn/Globe, tap and release it once.', 'neutral');
  syncControls();
}

/** Cancels an in-progress hotkey capture. */
export function cancelDictationHotkeyCapture() {
  if (!state.isCapturingDictationHotkey) return;
  state.isCapturingDictationHotkey = false;
  if (state.savedDictationHotkey) {
    setDictationHotkeyStatus(`Current hotkey: ${state.savedDictationHotkey}. ${state.dictationTriggerStatus}`, 'ok');
  } else {
    setDictationHotkeyStatus(`Hotkey disabled. Default: ${state.defaultDictationHotkey}.`, 'neutral');
  }
  syncControls();
}

/**
 * Persists a hotkey combo via Tauri.
 * @param {string} value
 */
export async function saveDictationHotkey(value) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isNativeDesktopMode()) return;

  const parsed = parseHotkeyCombo(value);
  if (!parsed.ok) {
    setDictationHotkeyStatus(parsed.error, 'error');
    setStatus(`Could not save hotkey: ${parsed.error}`, 'error');
    return;
  }

  try {
    const payload = await tauriInvoke('set_dictation_trigger', { trigger: parsed.display });
    applyDictationHotkeyPayload(payload);
    setStatus(`Dictation hotkey saved: ${parsed.display}`, 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not save hotkey: ${details}`, 'error');
    setStatus(`Could not save hotkey: ${details}`, 'error');
  } finally {
    state.isCapturingDictationHotkey = false;
    syncControls();
  }
}

/** Clears the saved dictation hotkey via Tauri. */
export async function clearDictationHotkey() {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isNativeDesktopMode()) return;

  try {
    const payload = await tauriInvoke('clear_dictation_trigger');
    applyDictationHotkeyPayload(payload);
    setStatus('Dictation hotkey disabled.', 'neutral');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not disable hotkey: ${details}`, 'error');
    setStatus(`Could not disable hotkey: ${details}`, 'error');
  } finally {
    state.isCapturingDictationHotkey = false;
    syncControls();
  }
}

/**
 * Handles key events while capturing a hotkey.
 * @param {KeyboardEvent} event
 * @returns {boolean}
 */
export function maybeCaptureDictationHotkeyEvent(event) {
  if (!state.isCapturingDictationHotkey) return false;

  const isCancel = event.type === 'keydown'
    && event.key === 'Escape'
    && !event.metaKey
    && !event.ctrlKey
    && !event.altKey
    && !event.shiftKey;
  if (isCancel) {
    event.preventDefault();
    cancelDictationHotkeyCapture();
    return true;
  }

  const capturedCombo = buildHotkeyFromEvent(event);
  if (!capturedCombo) return false;

  if (event.type === 'keyup' && capturedCombo !== 'Fn') {
    return false;
  }

  event.preventDefault();
  const parsed = parseHotkeyCombo(capturedCombo);
  if (!parsed.ok) {
    setDictationHotkeyStatus(parsed.error, 'error');
    return true;
  }

  state.pendingDictationHotkey = parsed.display;
  state.isCapturingDictationHotkey = false;
  setDictationHotkeyStatus(`Pending hotkey: ${parsed.display}. Click "Save Hotkey" to apply.`, 'neutral');
  syncControls();
  return true;
}

/**
 * Top-level document hotkey handler (capture mode only on this path).
 * @param {KeyboardEvent} event
 */
export function handleDictationHotkeyEvent(event) {
  if (!isNativeDesktopMode()) return;

  if (maybeCaptureDictationHotkeyEvent(event)) {
    return;
  }
}
