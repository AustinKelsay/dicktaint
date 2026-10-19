/** Dictate-into-focused-field setting. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import { getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage } from '../platform.js';
import { setStatus, syncControls } from '../ui.js';

/**
 * Serializes focused-field inserts so overlapping Cmd+V / pasteboard cycles
 * cannot paste a prior transcript into the focused field.
 */
let focusedFieldInsertChain = Promise.resolve();

export function setFocusedFieldInsertStatus(message, tone = 'neutral') {
  if (!dom.focusedFieldInsertStatusEl) return;
  dom.focusedFieldInsertStatusEl.textContent = message;
  dom.focusedFieldInsertStatusEl.dataset.tone = tone;
}


export function applyFocusedFieldInsertPayload(payload) {
  const enabled = Boolean(
    payload?.focused_field_insert_enabled
      ?? payload?.focusedFieldInsertEnabled
      ?? payload?.enabled
  );
  state.focusedFieldInsertPermissionGranted = Boolean(
    payload?.focused_field_insert_permission_granted
      ?? payload?.focusedFieldInsertPermissionGranted
      ?? payload?.permission_granted
  );
  state.focusedFieldInsertPermissionStatus = String(
    (
      payload?.focused_field_insert_permission_status
      ?? payload?.focusedFieldInsertPermissionStatus
      ?? payload?.permission_status
    ) || ''
  ).trim();
  state.focusedFieldInsertEnabled = enabled;

  if (dom.focusedFieldInsertToggleEl) {
    dom.focusedFieldInsertToggleEl.checked = enabled;
  }

  if (enabled && state.focusedFieldInsertPermissionGranted) {
    setFocusedFieldInsertStatus(
      state.focusedFieldInsertPermissionStatus || 'On. Finished dictations are pasted where you’re typing.',
      'ok'
    );
  } else if (enabled) {
    setFocusedFieldInsertStatus(
      state.focusedFieldInsertPermissionStatus || 'This needs Accessibility access. Allow Murmur in System Settings, then recheck.',
      'error'
    );
  } else {
    setFocusedFieldInsertStatus(
      state.focusedFieldInsertPermissionStatus || 'Off. Dictations stay in Murmur’s draft.',
      'neutral'
    );
  }
  syncControls();
}


export async function saveFocusedFieldInsertSetting(enabled) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  try {
    state.isSavingFocusedFieldInsertSetting = true;
    syncControls();
    const payload = await tauriInvoke('set_focused_field_insert_enabled', { enabled: Boolean(enabled) });
    applyFocusedFieldInsertPayload(payload);
    setStatus(
      state.focusedFieldInsertEnabled
        ? (state.focusedFieldInsertPermissionGranted
          ? 'Finished dictations will now be pasted where you’re typing.'
          : (state.focusedFieldInsertPermissionStatus || 'This needs Accessibility access. Allow Murmur in System Settings, then recheck.'))
        : 'Dictations will stay in Murmur’s draft.',
      state.focusedFieldInsertEnabled && !state.focusedFieldInsertPermissionGranted ? 'error' : 'ok'
    );
  } catch (error) {
    const details = getErrorMessage(error);
    if (dom.focusedFieldInsertToggleEl) {
      dom.focusedFieldInsertToggleEl.checked = state.focusedFieldInsertEnabled;
    }
    setFocusedFieldInsertStatus(`Could not save this setting: ${details}`, 'error');
    setStatus(`Could not save the typing setting: ${details}`, 'error');
  } finally {
    state.isSavingFocusedFieldInsertSetting = false;
    syncControls();
  }
}


/**
 * Queues a focused-field insert so concurrent transcripts paste in order.
 *
 * Fire-and-forget callers (`void maybeInsert…`) still benefit: each insert waits
 * for the previous pasteboard write / Cmd+V / restore cycle to finish.
 *
 * @param {string} chunk Transcript text to paste into the frontmost field.
 * @returns {Promise<void>|undefined}
 */
export async function maybeInsertTranscriptIntoFocusedField(chunk) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode() || !state.focusedFieldInsertEnabled) return;
  if (typeof document.hasFocus === 'function' && document.hasFocus()) return;

  const trimmed = String(chunk || '').trim();
  if (!trimmed) return;

  async function runInsert() {
    try {
      await tauriInvoke('insert_text_into_focused_field', { text: trimmed });
    } catch (error) {
      const details = getErrorMessage(error);
      setFocusedFieldInsertStatus(`Pasting into the other app failed: ${details}`, 'error');
      setStatus(`Your words are in the draft, but pasting into the other app failed: ${details}`, 'error');
    }
  }

  const next = focusedFieldInsertChain.then(runInsert, runInsert);
  focusedFieldInsertChain = next.then(
    () => undefined,
    () => undefined
  );
  return next;
}

/** Clears the insert queue so frontend tests start from an idle chain. */
export function resetFocusedFieldInsertQueueForTests() {
  focusedFieldInsertChain = Promise.resolve();
}

