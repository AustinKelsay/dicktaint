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
    setStatus('Floating pill setting saved.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    syncBackgroundUiControls();
    setBackgroundUiStatus(`Could not save the floating pill setting: ${details}`, 'error');
    setStatus(`Could not save the floating pill setting: ${details}`, 'error');
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
    setStatus('Menu bar setting saved.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    syncBackgroundUiControls();
    setBackgroundUiStatus(`Could not save the menu bar setting: ${details}`, 'error');
    setStatus(`Could not save the menu bar setting: ${details}`, 'error');
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
    setStatus('Close button setting saved.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    syncBackgroundUiControls();
    setBackgroundUiStatus(`Could not save the close button setting: ${details}`, 'error');
    setStatus(`Could not save the close button setting: ${details}`, 'error');
  } finally {
    state.isSavingBackgroundUiSettings = false;
    syncControls();
  }
}
