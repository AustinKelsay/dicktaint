import { isNativeSetupReady, microphoneRecoveryAction } from '../readiness.js';
/**
 * Hotkey preset chips and permission guidance UI.
 */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import { DEFAULT_DICTATION_HOTKEY } from '../constants.js';
import { isNativeDesktopMode, isFocusedMacDesktopMode } from '../platform.js';
import {
  parseHotkeyCombo, getSuggestedHotkeyOptions, describeMachineLabel
} from './hotkey-logic.js';

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

/** Renders microphone / hotkey / accessibility permission guidance. */
export function renderPermissionGuidance() {
  if (!dom.dictationPermissionsCardEl || !dom.dictationPermissionSummaryEl || !dom.dictationPermissionListEl) return;

  dom.dictationPermissionsCardEl.hidden = !isNativeDesktopMode();
  if (!isNativeDesktopMode()) return;

  const machineLabel = describeMachineLabel();
  if (!isFocusedMacDesktopMode()) {
    dom.dictationPermissionSummaryEl.textContent = `${machineLabel} can’t dictate natively. Murmur’s dictation engine runs on macOS.`;
    dom.dictationPermissionListEl.innerHTML = '';
    return;
  }

  dom.dictationPermissionSummaryEl.textContent = isNativeSetupReady()
    ? 'You’re ready to dictate. These optional permissions add background Fn and typing into other apps.'
    : 'Finish the three checks above to start dictating. Everything here is optional.';
  const facts = state.macosPermissions;
  const microphone = facts?.microphone || 'unknown';
  const descriptions = {
    unknown: 'Microphone: status unknown. Recheck to try again.',
    not_determined: 'Microphone: not allowed yet. Murmur asks macOS once.',
    authorized: 'Microphone: allowed.',
    denied: 'Microphone: access denied. Allow Murmur in System Settings, then recheck.',
    restricted: 'Microphone: restricted by a system policy. Ask your device administrator.',
    unavailable: 'Microphone: couldn’t be checked. Recheck to try again.'
  };
  const items = [
    { text: descriptions[microphone], permission: 'microphone', action: microphoneRecoveryAction(microphone) },
    { text: `Accessibility: ${facts ? (facts.accessibility ? 'allowed' : 'not granted') : 'unknown'}. Lets Murmur paste into other apps and hear Fn while you’re in them.`, permission: 'accessibility', action: 'settings' },
    { text: `Input Monitoring: ${facts ? (facts.input_monitoring ? 'allowed' : 'not granted') : 'unknown'}. Another way to hear Fn in the background. Not needed when Accessibility is allowed.`, permission: 'input_monitoring', action: 'settings' },
    { text: state.dictationTriggerMode === 'global-hold' ? 'Background Fn is on. Hold Fn in another app to try it.' : 'The button always works without these. Fn also works while Murmur is in front.', action: 'recheck' }
  ];
  dom.dictationPermissionListEl.innerHTML = '';
  for (const item of items) {
    const li = document.createElement('li');
    li.textContent = item.text;
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'ghost quiet';
    button.textContent = item.action === 'allow' ? (state.isRequestingMicrophone ? 'Waiting for macOS…' : 'Allow microphone') : item.action === 'settings' ? 'Open System Settings' : 'Recheck permissions';
    button.disabled = state.isRequestingMicrophone;
    button.dataset.permissionAction = item.action;
    button.dataset.permission = item.permission || '';
    li.appendChild(button);
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
  if (dom.dictationPermissionsCardEl) dom.dictationPermissionsCardEl.hidden = !nativeDesktop;
  if (!nativeDesktop) return;

  if (dom.dictationHotkeyInputEl) {
    dom.dictationHotkeyInputEl.value = state.pendingDictationHotkey || state.savedDictationHotkey || '';
    dom.dictationHotkeyInputEl.placeholder = state.defaultDictationHotkey || DEFAULT_DICTATION_HOTKEY;
  }
  if (dom.recordDictationHotkeyBtn) {
    dom.recordDictationHotkeyBtn.textContent = state.isCapturingDictationHotkey ? 'Press keys…' : 'Record keys';
  }
  renderDictationHotkeyPresets();
  renderPermissionGuidance();
}
