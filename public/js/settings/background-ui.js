/** Menu bar, close action, and floating pill preferences. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import {
  DEFAULT_PILL_VISIBILITY_MODE, DEFAULT_MENU_BAR_MODE, DEFAULT_CLOSE_ACTION
} from '../constants.js';
import { getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage } from '../platform.js';
import { setStatus, syncControls } from '../ui.js';

export function setBackgroundUiStatus(message, tone = 'neutral') {
  if (!dom.backgroundUiStatusEl) return;
  dom.backgroundUiStatusEl.textContent = message;
  dom.backgroundUiStatusEl.dataset.tone = tone;
}


export function normalizePillVisibilityMode(value) {
  const normalized = String(value || '').trim().toLowerCase();
  if (normalized === 'off') return 'off';
  if (normalized === 'always') return 'always';
  return DEFAULT_PILL_VISIBILITY_MODE;
}


export function normalizeMenuBarMode(value) {
  const normalized = String(value || '').trim().toLowerCase();
  if (normalized === 'background-only') return 'background-only';
  if (normalized === 'off') return 'off';
  return DEFAULT_MENU_BAR_MODE;
}


export function normalizeCloseAction(value, nextMenuBarMode = state.menuBarMode) {
  if (normalizeMenuBarMode(nextMenuBarMode) === 'off') return 'quit';
  const normalized = String(value || '').trim().toLowerCase();
  return normalized === 'quit' ? 'quit' : DEFAULT_CLOSE_ACTION;
}


export function describeBackgroundUiStatus() {
  const trayText = state.menuBarMode === 'always'
    ? 'Menu bar: always visible.'
    : (state.menuBarMode === 'background-only'
      ? 'Menu bar: only visible while the main window is hidden.'
      : 'Menu bar: off.');
  const closeText = state.closeAction === 'hide-to-tray'
    ? 'Close button hides to the menu bar.'
    : 'Close button quits the app.';
  const pillText = state.pillVisibilityMode === 'always'
    ? 'Floating pill: always visible.'
    : (state.pillVisibilityMode === 'off'
      ? 'Floating pill: off.'
      : 'Floating pill: visible only while active.');
  return `${trayText} ${closeText} ${pillText}`;
}


export function syncBackgroundUiControls() {
  if (!dom.backgroundUiCardEl) return;

  const visible = isFocusedMacDesktopMode();
  dom.backgroundUiCardEl.hidden = !visible;
  if (!visible) return;

  if (dom.menuBarModeSelectEl) {
    dom.menuBarModeSelectEl.value = normalizeMenuBarMode(state.menuBarMode);
    dom.menuBarModeSelectEl.disabled = !visible || state.isSavingBackgroundUiSettings;
  }
  if (dom.closeActionSelectEl) {
    dom.closeActionSelectEl.value = normalizeCloseAction(state.closeAction, state.menuBarMode);
    dom.closeActionSelectEl.disabled = !visible || state.isSavingBackgroundUiSettings || state.menuBarMode === 'off';
  }
  if (dom.pillVisibilityModeSelectEl) {
    dom.pillVisibilityModeSelectEl.value = normalizePillVisibilityMode(state.pillVisibilityMode);
    dom.pillVisibilityModeSelectEl.disabled = !visible || state.isSavingBackgroundUiSettings;
  }

  setBackgroundUiStatus(
    describeBackgroundUiStatus(),
    state.menuBarMode === 'off' && state.closeAction === 'quit' ? 'neutral' : 'ok'
  );
}


export function applyBackgroundUiPreferencesPayload(payload) {
  state.menuBarMode = normalizeMenuBarMode(
    payload?.menu_bar_mode
      ?? payload?.menuBarMode
      ?? state.menuBarMode
  );
  state.closeAction = normalizeCloseAction(
    payload?.close_action
      ?? payload?.closeAction
      ?? state.closeAction,
    state.menuBarMode
  );
  state.pillVisibilityMode = normalizePillVisibilityMode(
    payload?.pill_visibility_mode
      ?? payload?.pillVisibilityMode
      ?? state.pillVisibilityMode
  );
  syncBackgroundUiControls();
  syncControls();
}


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

