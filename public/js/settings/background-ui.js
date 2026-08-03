/**
 * Menu bar, close action, and floating pill preference persistence.
 *
 * Sync/normalize helpers live in background-ui-controls.js (cycle-free leaf).
 */
import { state } from '../state.js';
import { getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage } from '../platform.js';
import { setStatus, syncControls } from '../ui.js';
import {
  setBackgroundUiStatus,
  normalizePillVisibilityMode,
  normalizeMenuBarMode,
  normalizeCloseAction,
  syncBackgroundUiControls,
  applyBackgroundUiPreferencesPayload
} from './background-ui-controls.js';

export {
  setBackgroundUiStatus,
  normalizePillVisibilityMode,
  normalizeMenuBarMode,
  normalizeCloseAction,
  describeBackgroundUiStatus,
  syncBackgroundUiControls,
  applyBackgroundUiPreferencesPayload
} from './background-ui-controls.js';

/**
 * @param {string} mode
 */
export async function savePillVisibilityMode(mode) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  try {
    state.isSavingBackgroundUiSettings = true;
    syncControls();
    const payload = await tauriInvoke('set_pill_visibility_mode', {
      mode: normalizePillVisibilityMode(mode)
    });
    applyBackgroundUiPreferencesPayload(payload);
    setStatus('Floating pill preference saved.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    syncBackgroundUiControls();
    setBackgroundUiStatus(`Could not save floating pill setting: ${details}`, 'error');
    setStatus(`Could not save floating pill setting: ${details}`, 'error');
  } finally {
    state.isSavingBackgroundUiSettings = false;
    syncControls();
  }
}

/**
 * @param {string} mode
 */
export async function saveMenuBarMode(mode) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  try {
    state.isSavingBackgroundUiSettings = true;
    syncControls();
    const payload = await tauriInvoke('set_menu_bar_mode', {
      mode: normalizeMenuBarMode(mode)
    });
    applyBackgroundUiPreferencesPayload(payload);
    setStatus('Menu bar preference saved.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    syncBackgroundUiControls();
    setBackgroundUiStatus(`Could not save menu bar setting: ${details}`, 'error');
    setStatus(`Could not save menu bar setting: ${details}`, 'error');
  } finally {
    state.isSavingBackgroundUiSettings = false;
    syncControls();
  }
}

/**
 * @param {string} nextAction
 */
export async function saveCloseAction(nextAction) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  try {
    state.isSavingBackgroundUiSettings = true;
    syncControls();
    const payload = await tauriInvoke('set_close_action', {
      action: normalizeCloseAction(nextAction, state.menuBarMode)
    });
    applyBackgroundUiPreferencesPayload(payload);
    setStatus('Close button behavior saved.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    syncBackgroundUiControls();
    setBackgroundUiStatus(`Could not save close button behavior: ${details}`, 'error');
    setStatus(`Could not save close button behavior: ${details}`, 'error');
  } finally {
    state.isSavingBackgroundUiSettings = false;
    syncControls();
  }
}
