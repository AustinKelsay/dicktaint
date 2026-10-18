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
    dom.dictationPermissionSummaryEl.textContent = `${machineLabel} is outside the supported macOS desktop path.`;
    dom.dictationPermissionListEl.innerHTML = '';
    return;
  }

  dom.dictationPermissionSummaryEl.textContent = isNativeSetupReady()
    ? 'Ready for local dictation. Optional permissions add background Fn and insertion.'
    : 'Complete the local model, speech engine, and microphone steps to start dictating.';
  const facts = state.macosPermissions;
  const microphone = facts?.microphone || 'unknown';
  const descriptions = {
    unknown: 'Microphone: status unknown. Retry the permission check.',
    not_determined: 'Microphone: allow access to record your voice.',
    authorized: 'Microphone: access granted.',
    denied: 'Microphone: access denied. Enable Murmur in System Settings, then return and recheck.',
    restricted: 'Microphone: restricted by system policy. Contact your device administrator.',
    unavailable: 'Microphone: status unavailable. Retry the check.'
  };
  const items = [
    { text: descriptions[microphone], permission: 'microphone', action: microphoneRecoveryAction(microphone) },
    { text: `Accessibility: ${facts ? (facts.accessibility ? 'granted' : 'not granted') : 'unknown'}. Optional for inserting text into other apps and background Fn.`, permission: 'accessibility', action: 'settings' },
    { text: `Input Monitoring: ${facts ? (facts.input_monitoring ? 'granted' : 'not granted') : 'unknown'}. Optional alternative for background Fn.`, permission: 'input_monitoring', action: 'settings' },
    { text: state.dictationTriggerMode === 'global-hold' ? 'Background Fn listener is armed. Try holding Fn in another app to verify delivery.' : 'Button dictation works without optional permissions. Fn can also work while Murmur is focused.', action: 'recheck' }
  ];
  dom.dictationPermissionListEl.innerHTML = '';
  for (const item of items) {
    const li = document.createElement('li');
    li.textContent = item.text;
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'ghost quiet';
    button.textContent = item.action === 'allow' ? (state.isRequestingMicrophone ? 'Waiting for macOS…' : 'Allow microphone') : item.action === 'settings' ? 'Open Settings' : 'Recheck permissions';
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
    dom.recordDictationHotkeyBtn.textContent = state.isCapturingDictationHotkey ? 'Press Keys...' : 'Record';
  }
  renderDictationHotkeyPresets();
  renderPermissionGuidance();
}
