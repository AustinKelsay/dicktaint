/** Dictation hotkey parsing, capture, and persistence. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import {
  DEFAULT_DICTATION_HOTKEY, HOTKEY_MODIFIER_ORDER, HOTKEY_PRESET_OPTIONS, NATIVE_HOLD_HOTKEYS
} from '../constants.js';
import {
  isMacPlatform, getTauriInvoke, isNativeDesktopMode, isFocusedMacDesktopMode, getErrorMessage
} from '../platform.js';
import { setStatus, syncControls } from '../ui.js';
import { syncDictationHotkeyUi } from './hotkey-ui.js';

export function setDictationHotkeyStatus(message, tone = 'neutral') {
  if (!dom.dictationHotkeyStatusEl) return;
  dom.dictationHotkeyStatusEl.textContent = message;
  dom.dictationHotkeyStatusEl.dataset.tone = tone;
}


export function normalizeHotkeyModifier(token) {
  const normalized = String(token || '').trim().toLowerCase();
  if (!normalized) return null;

  if (['cmdorctrl', 'commandorcontrol', 'primary', 'mod'].includes(normalized)) return 'CmdOrCtrl';
  if (['cmd', 'command'].includes(normalized)) return 'Cmd';
  if (['ctrl', 'control'].includes(normalized)) return 'Ctrl';
  if (['alt', 'option'].includes(normalized)) return 'Alt';
  if (normalized === 'shift') return 'Shift';
  if (['super', 'meta', 'win', 'windows'].includes(normalized)) return 'Super';
  return null;
}


export function normalizeHotkeyKey(token) {
  const trimmed = String(token || '').trim();
  if (!trimmed) return null;

  if (/^[a-z0-9]$/i.test(trimmed)) return trimmed.toUpperCase();

  const lower = trimmed.toLowerCase();
  const aliasMap = {
    fn: 'Fn',
    function: 'Fn',
    globe: 'Fn',
    space: 'Space',
    tab: 'Tab',
    enter: 'Enter',
    return: 'Enter',
    esc: 'Escape',
    escape: 'Escape',
    backspace: 'Backspace',
    delete: 'Delete',
    del: 'Delete',
    up: 'Up',
    arrowup: 'Up',
    down: 'Down',
    arrowdown: 'Down',
    left: 'Left',
    arrowleft: 'Left',
    right: 'Right',
    arrowright: 'Right',
    home: 'Home',
    end: 'End',
    pageup: 'PageUp',
    pagedown: 'PageDown',
    insert: 'Insert'
  };
  if (aliasMap[lower]) return aliasMap[lower];

  const functionKeyMatch = /^f([1-9]|1\d|2[0-4])$/i.exec(trimmed);
  if (functionKeyMatch) {
    return `F${functionKeyMatch[1]}`;
  }

  return null;
}


export function parseHotkeyCombo(raw) {
  const source = String(raw || '').trim();
  if (!source) {
    return { ok: false, error: 'Hotkey cannot be empty.' };
  }

  const modifiers = new Set();
  let key = null;

  for (const token of source.split('+')) {
    const trimmed = token.trim();
    if (!trimmed) {
      return { ok: false, error: 'Hotkey contains an empty token.' };
    }

    const modifier = normalizeHotkeyModifier(trimmed);
    if (modifier) {
      if (key) return { ok: false, error: 'Modifiers must come before the main key.' };
      modifiers.add(modifier);
      continue;
    }

    if (key) {
      return { ok: false, error: 'Hotkey can only have one main key.' };
    }
    key = normalizeHotkeyKey(trimmed);
    if (!key) {
      return { ok: false, error: `Unsupported key "${trimmed}".` };
    }
  }

  if (key === 'Fn') {
    if (modifiers.size) return { ok: false, error: 'Fn hotkey must be used by itself.' };
    return {
      ok: true,
      display: 'Fn',
      key: 'Fn',
      requires: {
        cmdOrCtrl: false,
        cmd: false,
        ctrl: false,
        alt: false,
        shift: false,
        super: false
      }
    };
  }

  if (!modifiers.size) return { ok: false, error: 'Hotkey must include at least one modifier (or use Fn by itself on macOS).' };
  if (!key) return { ok: false, error: 'Hotkey is missing its main key.' };
  if (modifiers.has('CmdOrCtrl') && (modifiers.has('Cmd') || modifiers.has('Ctrl'))) {
    return { ok: false, error: 'Use CmdOrCtrl by itself, or use Cmd/Ctrl explicitly.' };
  }

  const orderedModifiers = HOTKEY_MODIFIER_ORDER.filter((modifier) => modifiers.has(modifier));
  const display = [...orderedModifiers, key].join('+');
  return {
    ok: true,
    display,
    key,
    requires: {
      cmdOrCtrl: modifiers.has('CmdOrCtrl'),
      cmd: modifiers.has('Cmd'),
      ctrl: modifiers.has('Ctrl'),
      alt: modifiers.has('Alt'),
      shift: modifiers.has('Shift'),
      super: modifiers.has('Super')
    }
  };
}


export function eventKeyToken(event) {
  const code = String(event?.code || '').trim();
  if (/^fn$/i.test(code)) return 'Fn';
  if (/^f19$/i.test(code)) return 'F19';
  return normalizeHotkeyKey(event.key);
}


export function buildHotkeyFromEvent(event) {
  const key = eventKeyToken(event);
  if (!key) return null;
  if (key === 'Fn') return 'Fn';

  const keyName = String(event.key || '').toLowerCase();
  if (['shift', 'control', 'meta', 'alt', 'super'].includes(keyName)) return null;

  const isMac = isMacPlatform();
  const modifiers = [];

  const primaryPressed = isMac ? event.metaKey : event.ctrlKey;
  if (primaryPressed) modifiers.push('CmdOrCtrl');
  if (event.altKey) modifiers.push('Alt');
  if (event.shiftKey) modifiers.push('Shift');

  if (!modifiers.length) return null;
  return [...modifiers, key].join('+');
}


export function eventMatchesHotkey(event, spec) {
  if (!spec?.ok) return false;
  if (event.repeat) return false;
  if (spec.key === 'Fn') {
    const key = eventKeyToken(event);
    const keyName = String(event.key || '').toLowerCase();
    const fnPressed = key === 'Fn'
      || keyName === 'fn'
      || (keyName === 'unidentified' && Boolean(event.getModifierState?.('Fn')));
    if (!fnPressed) return false;
    return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
  }

  const key = eventKeyToken(event);
  if (!key || key !== spec.key) return false;

  const isMac = isMacPlatform();
  const expectedCtrl = Boolean(spec.requires.ctrl || (spec.requires.cmdOrCtrl && !isMac));
  const expectedMeta = Boolean(
    spec.requires.cmd
      || spec.requires.super
      || (spec.requires.cmdOrCtrl && isMac)
  );

  if (event.ctrlKey !== expectedCtrl) return false;
  if (event.metaKey !== expectedMeta) return false;
  if (event.altKey !== Boolean(spec.requires.alt)) return false;
  if (event.shiftKey !== Boolean(spec.requires.shift)) return false;
  return true;
}


export function getSuggestedHotkeyOptions() {
  if (isFocusedMacDesktopMode()) {
    return HOTKEY_PRESET_OPTIONS;
  }
  return HOTKEY_PRESET_OPTIONS.filter((option) => option.value !== 'Fn');
}


export function humanizeArchitecture(arch) {
  const value = String(arch || '').trim().toLowerCase();
  if (['aarch64', 'arm64'].includes(value)) return 'Apple silicon';
  if (['x86_64', 'x64', 'amd64'].includes(value)) return 'Intel';
  return arch || 'unknown arch';
}


export function describeMachineLabel(device = state.currentDeviceProfile) {
  if (!device) return 'This Mac';
  if (String(device.os || '').toLowerCase() === 'macos') {
    const arch = humanizeArchitecture(device.architecture);
    return arch === 'Apple silicon' ? 'This Apple silicon Mac' : (arch === 'Intel' ? 'This Intel Mac' : 'This Mac');
  }
  return 'This device';
}


export function instructionHotkeyLabel(raw = state.savedDictationHotkey || state.pendingDictationHotkey || state.defaultDictationHotkey) {
  const value = String(raw || '').trim();
  if (!value) return 'a hotkey';
  return value === 'Fn' ? 'Fn / Globe' : value;
}


export function isHoldToTalkHotkey() {
  return state.dictationTriggerMode === 'global-hold'
    || state.dictationTriggerMode === 'focused-window-hold'
    || state.activeHotkeySpec?.key === 'Fn';
}


export function idlePillMessage() {
  if (!isNativeDesktopMode()) return '';
  if (!isFocusedMacDesktopMode()) return 'Desktop MVP: macOS only';
  if (!state.currentOnboarding) return 'Checking dictation setup...';
  if (!state.nativeDictationModelReady) return 'Finish setup in dicktaint';
  if (!state.savedDictationHotkey) return 'Hotkey disabled - open settings';
  if (state.dictationTriggerMode === 'focused-window-hold') {
    return `Focus dicktaint, then hold ${instructionHotkeyLabel(state.savedDictationHotkey)}`;
  }
  if (isHoldToTalkHotkey()) {
    return `Hold ${instructionHotkeyLabel(state.savedDictationHotkey)} to dictate`;
  }
  return `Press ${instructionHotkeyLabel(state.savedDictationHotkey)} to dictate`;
}


export function listeningStatusForTrigger(trigger) {
  if (trigger === 'hotkey-hold' || isHoldToTalkHotkey()) {
    return `Listening... release ${instructionHotkeyLabel(state.savedDictationHotkey)} to stop and transcribe.`;
  }
  if (trigger === 'hotkey') {
    return `Listening... press ${instructionHotkeyLabel(state.savedDictationHotkey)} again to stop and transcribe.`;
  }
  return 'Listening... click Stop to transcribe.';
}


export function completedStatusForTrigger(trigger) {
  if (trigger === 'hotkey-hold' || isHoldToTalkHotkey()) {
    return `Dictation captured from ${instructionHotkeyLabel(state.savedDictationHotkey)} hold and transcribed.`;
  }
  if (trigger === 'hotkey') {
    return `Dictation captured from ${instructionHotkeyLabel(state.savedDictationHotkey)} and transcribed.`;
  }
  return 'Dictation captured and transcribed.';
}


export function applyDictationHotkeyPayload(payload, { preservePending = false } = {}) {
  state.dictationTriggerMode = String(
    payload?.dictation_trigger_mode
      || payload?.trigger_mode
      || 'disabled'
  ).trim() || 'disabled';
  state.dictationTriggerStatus = String(
    payload?.dictation_trigger_status
      || payload?.trigger_status
      || 'Hotkey disabled.'
  ).trim() || 'Hotkey disabled.';
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
    setDictationHotkeyStatus(`Hotkey disabled. Default: ${state.defaultDictationHotkey}.`, 'neutral');
    syncControls();
    return;
  }

  const parsedTrigger = parseHotkeyCombo(rawTrigger);
  if (!parsedTrigger.ok) {
    state.savedDictationHotkey = null;
    state.activeHotkeySpec = null;
    if (!preservePending) state.pendingDictationHotkey = '';
    setDictationHotkeyStatus(`Saved hotkey ignored: ${parsedTrigger.error}`, 'error');
    syncControls();
    return;
  }

  state.savedDictationHotkey = parsedTrigger.display;
  state.activeHotkeySpec = parsedTrigger;
  if (!preservePending) state.pendingDictationHotkey = parsedTrigger.display;
  setDictationHotkeyStatus(`Current hotkey: ${parsedTrigger.display}. ${state.dictationTriggerStatus}`, 'ok');
  syncControls();
}


export function beginDictationHotkeyCapture() {
  if (!isNativeDesktopMode()) return;
  state.isCapturingDictationHotkey = true;
  setDictationHotkeyStatus('Press your desired key combo now. Press Escape to cancel. For Fn/Globe, tap and release it once.', 'neutral');
  syncControls();
}


export function cancelDictationHotkeyCapture() {
  if (!state.isCapturingDictationHotkey) return;
  state.isCapturingDictationHotkey = false;
  if (state.savedDictationHotkey) {
    setDictationHotkeyStatus(`Current hotkey: ${state.savedDictationHotkey}. ${state.dictationTriggerStatus}`, 'ok');
  } else {
    setDictationHotkeyStatus(`Hotkey disabled. Default: ${state.defaultDictationHotkey}.`, 'neutral');
  }
  syncControls();
}


export async function saveDictationHotkey(value) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isNativeDesktopMode()) return;

  const parsed = parseHotkeyCombo(value);
  if (!parsed.ok) {
    setDictationHotkeyStatus(parsed.error, 'error');
    setStatus(`Could not save hotkey: ${parsed.error}`, 'error');
    return;
  }

  try {
    const payload = await tauriInvoke('set_dictation_trigger', { trigger: parsed.display });
    applyDictationHotkeyPayload(payload);
    setStatus(`Dictation hotkey saved: ${parsed.display}`, 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not save hotkey: ${details}`, 'error');
    setStatus(`Could not save hotkey: ${details}`, 'error');
  } finally {
    state.isCapturingDictationHotkey = false;
    syncControls();
  }
}


export async function clearDictationHotkey() {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isNativeDesktopMode()) return;

  try {
    const payload = await tauriInvoke('clear_dictation_trigger');
    applyDictationHotkeyPayload(payload);
    setStatus('Dictation hotkey disabled.', 'neutral');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not disable hotkey: ${details}`, 'error');
    setStatus(`Could not disable hotkey: ${details}`, 'error');
  } finally {
    state.isCapturingDictationHotkey = false;
    syncControls();
  }
}


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
  setDictationHotkeyStatus(`Pending hotkey: ${parsed.display}. Click "Save Hotkey" to apply.`, 'neutral');
  syncControls();
  return true;
}


export function handleDictationHotkeyEvent(event) {
  if (!isNativeDesktopMode()) return;

  if (maybeCaptureDictationHotkeyEvent(event)) {
    return;
  }
}

