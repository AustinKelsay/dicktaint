/**
 * Hotkey preset chips and permission guidance UI.
 */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import { DEFAULT_DICTATION_HOTKEY } from '../constants.js';
import { isNativeDesktopMode, isFocusedMacDesktopMode } from '../platform.js';
import { parseHotkeyCombo, getSuggestedHotkeyOptions } from './hotkey-logic.js';

/** Renders suggested hotkey preset chips. */
export function renderDictationHotkeyPresets() {
  if (!dom.dictationHotkeyPresetsEl) return;
  dom.dictationHotkeyPresetsEl.innerHTML = '';
  const activeValue = String(state.pendingDictationHotkey || state.savedDictationHotkey || '').trim();
  const normalizedActive = parseHotkeyCombo(activeValue);

  for (const option of getSuggestedHotkeyOptions()) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'ghost quiet preset-chip';
    button.textContent = option.label;
    button.dataset.hotkeyPreset = option.value;
    if (normalizedActive.ok && normalizedActive.display === option.value) {
      button.className += ' is-active';
    }
    dom.dictationHotkeyPresetsEl.appendChild(button);
  }
}

/**
 * Renders the optional Mac access rows. The microphone lives in the required setup checks,
 * and the shortcut card already says whether Fn works from any app.
 */
export function renderPermissionGuidance() {
  if (!dom.dictationPermissionsCardEl || !dom.dictationPermissionListEl) return;
  dom.dictationPermissionsCardEl.hidden = !isFocusedMacDesktopMode();
  if (!isFocusedMacDesktopMode()) return;

  const facts = state.macosPermissions;
  const grant = (allowed) => (facts ? (allowed ? 'allowed' : 'not allowed') : 'unknown');
  const items = [
    { text: `Accessibility · ${grant(facts?.accessibility)}. Pastes into other apps and hears Fn from any app.`, permission: 'accessibility', allowed: facts?.accessibility },
    // Input Monitoring is only an alternative Fn route, so it drops away once Accessibility covers it.
    ...(facts?.accessibility ? [] : [{ text: `Input Monitoring · ${grant(facts?.input_monitoring)}. Hears Fn from any app, without pasting.`, permission: 'input_monitoring', allowed: facts?.input_monitoring }])
  ];
  dom.dictationPermissionListEl.innerHTML = '';
  for (const item of items) {
    const li = document.createElement('li');
    li.textContent = item.text;
    if (!item.allowed) {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'ghost quiet';
      button.textContent = 'Open System Settings';
      button.dataset.permissionAction = 'settings';
      button.dataset.permission = item.permission;
      li.appendChild(button);
    }
    dom.dictationPermissionListEl.appendChild(li);
  }
  if (state.macosPermissionError) {
    const li = document.createElement('li');
    li.textContent = state.macosPermissionError;
    li.dataset.tone = 'error';
    dom.dictationPermissionListEl.appendChild(li);
  }
}

/** Syncs hotkey card visibility, input value, presets, and permission guidance. */
export function syncDictationHotkeyUi() {
  const nativeDesktop = isNativeDesktopMode();
  if (dom.dictationHotkeyCardEl) dom.dictationHotkeyCardEl.hidden = !nativeDesktop;
  if (dom.focusedFieldInsertCardEl) dom.focusedFieldInsertCardEl.hidden = !nativeDesktop;
  renderPermissionGuidance();
  if (!nativeDesktop) return;

  if (dom.dictationHotkeyInputEl) {
    dom.dictationHotkeyInputEl.value = state.pendingDictationHotkey || state.savedDictationHotkey || '';
    dom.dictationHotkeyInputEl.placeholder = state.defaultDictationHotkey || DEFAULT_DICTATION_HOTKEY;
  }
  if (dom.recordDictationHotkeyBtn) {
    dom.recordDictationHotkeyBtn.textContent = state.isCapturingDictationHotkey ? 'Press keys…' : 'Record keys';
  }
  renderDictationHotkeyPresets();
}
