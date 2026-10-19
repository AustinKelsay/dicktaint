/**
 * Background UI control sync (leaf — breaks ui.js ↔ background-ui.js cycle).
 *
 * Layering: background-ui-controls ← ui | background-ui | onboarding
 */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import {
  DEFAULT_PILL_VISIBILITY_MODE, DEFAULT_MENU_BAR_MODE, DEFAULT_CLOSE_ACTION
} from '../constants.js';
import { isFocusedMacDesktopMode } from '../platform.js';

/**
 * @param {string} message
 * @param {string} [tone]
 */
export function setBackgroundUiStatus(message, tone = 'neutral') {
  if (!dom.backgroundUiStatusEl) return;
  dom.backgroundUiStatusEl.textContent = message;
  dom.backgroundUiStatusEl.dataset.tone = tone;
}

/**
 * @param {unknown} value
 * @returns {'off' | 'always' | string}
 */
export function normalizePillVisibilityMode(value) {
  const normalized = String(value || '').trim().toLowerCase();
  if (normalized === 'off') return 'off';
  if (normalized === 'always') return 'always';
  return DEFAULT_PILL_VISIBILITY_MODE;
}

/**
 * @param {unknown} value
 * @returns {'background-only' | 'off' | string}
 */
export function normalizeMenuBarMode(value) {
  const normalized = String(value || '').trim().toLowerCase();
  if (normalized === 'background-only') return 'background-only';
  if (normalized === 'off') return 'off';
  return DEFAULT_MENU_BAR_MODE;
}

/**
 * @param {unknown} value
 * @param {unknown} [nextMenuBarMode]
 * @returns {'quit' | string}
 */
export function normalizeCloseAction(value, nextMenuBarMode = state.menuBarMode) {
  if (normalizeMenuBarMode(nextMenuBarMode) === 'off') return 'quit';
  const normalized = String(value || '').trim().toLowerCase();
  return normalized === 'quit' ? 'quit' : DEFAULT_CLOSE_ACTION;
}

/** @returns {string} */
export function describeBackgroundUiStatus() {
  const trayText = state.menuBarMode === 'always'
    ? 'The menu bar icon is always shown.'
    : (state.menuBarMode === 'background-only'
      ? 'The menu bar icon appears only while the window is hidden.'
      : 'No menu bar icon.');
  const closeText = state.closeAction === 'hide-to-tray'
    ? 'Closing the window keeps Murmur running in the menu bar.'
    : 'Closing the window quits Murmur.';
  const pillText = state.pillVisibilityMode === 'always'
    ? 'The floating pill is always shown.'
    : (state.pillVisibilityMode === 'off'
      ? 'The floating pill is off.'
      : 'The floating pill appears while dictating.');
  return `${trayText} ${closeText} ${pillText}`;
}

/** Syncs menu bar / close / pill controls from state. */
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

/**
 * Applies tray/pill preference fields from an onboarding/settings payload.
 * Callers are responsible for syncControls() when broader control state must refresh.
 * @param {object} payload
 */
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
}
