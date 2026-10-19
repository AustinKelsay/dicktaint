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
  buildHotkeyFromEvent,
  humanizeHotkeyLabel
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
  humanizeHotkeyLabel,
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
      || 'Shortcut off.'
  ).trim() || 'Shortcut off.';
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
    setDictationHotkeyStatus(shortcutOffStatus(), 'neutral');
    syncControls();
    return;
  }

  const parsedTrigger = parseHotkeyCombo(rawTrigger);
  if (!parsedTrigger.ok) {
    state.savedDictationHotkey = null;
    state.activeHotkeySpec = null;
    if (!preservePending) state.pendingDictationHotkey = '';
    setDictationHotkeyStatus(`The saved shortcut can’t be used: ${parsedTrigger.error}`, 'error');
    syncControls();
    return;
  }

  state.savedDictationHotkey = parsedTrigger.display;
  state.activeHotkeySpec = parsedTrigger;
  if (!preservePending) state.pendingDictationHotkey = parsedTrigger.display;
  setDictationHotkeyStatus(shortcutSavedStatus(parsedTrigger.display), 'ok');
  syncControls();
}

/** @returns {string} */
function shortcutOffStatus() {
  return 'Shortcut off. Pick one above, or use the Start button.';
}

/**
 * @param {string} display Canonical saved shortcut.
 * @returns {string}
 */
function shortcutSavedStatus(display) {
  return `Shortcut saved: ${humanizeHotkeyLabel(display)}. ${state.dictationTriggerStatus}`.trim();
}

/** Starts keyboard capture for a new hotkey combo. */
export function beginDictationHotkeyCapture() {
  if (!isNativeDesktopMode()) return;
  state.isCapturingDictationHotkey = true;
  setDictationHotkeyStatus('Press the keys you want. Tap Fn once for hold-to-talk. Esc cancels.', 'neutral');
  syncControls();
}

/** Cancels an in-progress hotkey capture. */
export function cancelDictationHotkeyCapture() {
  if (!state.isCapturingDictationHotkey) return;
  state.isCapturingDictationHotkey = false;
  if (state.savedDictationHotkey) {
    setDictationHotkeyStatus(shortcutSavedStatus(state.savedDictationHotkey), 'ok');
  } else {
    setDictationHotkeyStatus(shortcutOffStatus(), 'neutral');
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
    setStatus(`Could not save the shortcut: ${parsed.error}`, 'error');
    return;
  }

  try {
    const payload = await tauriInvoke('set_dictation_trigger', { trigger: parsed.display });
    applyDictationHotkeyPayload(payload);
    setStatus(`Shortcut saved: ${humanizeHotkeyLabel(parsed.display)}.`, 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not save the shortcut: ${details}`, 'error');
    setStatus(`Could not save the shortcut: ${details}`, 'error');
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
    setStatus('Shortcut turned off. The button still works.', 'neutral');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not turn off the shortcut: ${details}`, 'error');
    setStatus(`Could not turn off the shortcut: ${details}`, 'error');
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
  setDictationHotkeyStatus(`Ready to save: ${humanizeHotkeyLabel(parsed.display)}.`, 'neutral');
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
