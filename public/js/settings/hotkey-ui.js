/** Hotkey preset chips and permission guidance UI. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import { isNativeDesktopMode, isFocusedMacDesktopMode } from '../platform.js';
import {
  parseHotkeyCombo, getSuggestedHotkeyOptions, describeMachineLabel,
  instructionHotkeyLabel, setDictationHotkeyStatus
} from './hotkeys.js';

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

  dom.dictationPermissionSummaryEl.textContent = state.nativeDictationModelReady
    ? `${machineLabel} is ready. ${state.dictationTriggerStatus}`
    : `${machineLabel} still needs local setup. ${state.dictationTriggerStatus}`;

  const items = [
    {
      tone: state.nativeDictationModelReady ? 'ok' : 'neutral',
      text: 'Microphone: macOS asks the first time you start dictation. If audio fails later, relaunch after changing permission.'
    }
  ];

  if (state.savedDictationHotkey) {
    if (state.dictationTriggerMode === 'focused-window-hold') {
      items.push({
        tone: 'warn',
        text: state.dictationTriggerPermissionHint || 'Input Monitoring is required for global Fn hold-to-talk. Without it, Fn only works while dicktaint is focused.'
      });
    } else if (state.dictationTriggerMode === 'global-hold') {
      items.push({
        tone: 'ok',
        text: 'Input Monitoring: global Fn hold-to-talk is active for this app while it is running.'
      });
    } else {
      items.push({
        tone: 'ok',
        text: `Hotkey: ${instructionHotkeyLabel(state.savedDictationHotkey)} is registered globally while dicktaint is running.`
      });
    }
  } else {
    items.push({
      tone: 'neutral',
      text: 'Hotkey: disabled. Open Settings if you want a system-wide trigger again.'
    });
  }

  items.push({
    tone: state.focusedFieldInsertEnabled
      ? (state.focusedFieldInsertPermissionGranted ? 'ok' : 'warn')
      : 'neutral',
    text: state.focusedFieldInsertEnabled
      ? (state.focusedFieldInsertPermissionStatus
        || 'Accessibility is required for Dictate Into Focused Field.')
      : 'Accessibility is only needed if you enable Dictate Into Focused Field.'
  });

  dom.dictationPermissionListEl.innerHTML = '';
  for (const item of items) {
    const li = document.createElement('li');
    li.dataset.tone = item.tone;
    li.textContent = item.text;
    dom.dictationPermissionListEl.appendChild(li);
  }
}


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

