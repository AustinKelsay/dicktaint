/**
 * Pure hotkey parse/label helpers (leaf — breaks ui ↔ hotkeys ↔ hotkey-ui cycles).
 *
 * Layering: hotkey-logic ← hotkeys | hotkey-ui | ui | native-dictation | models
 */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import {
  HOTKEY_MODIFIER_ORDER, HOTKEY_PRESET_OPTIONS
} from '../constants.js';
import { isMacPlatform, isNativeDesktopMode, isFocusedMacDesktopMode } from '../platform.js';

/**
 * Writes status text into the hotkey status element.
 * @param {string} message
 * @param {string} [tone]
 */
export function setDictationHotkeyStatus(message, tone = 'neutral') {
  if (!dom.dictationHotkeyStatusEl) return;
  dom.dictationHotkeyStatusEl.textContent = message;
  dom.dictationHotkeyStatusEl.dataset.tone = tone;
}

/**
 * @param {unknown} token
 * @returns {string | null}
 */
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

/**
 * @param {unknown} token
 * @returns {string | null}
 */
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

/**
 * @param {unknown} raw
 * @returns {{ ok: true, display: string, key: string, requires: object } | { ok: false, error: string }}
 */
export function parseHotkeyCombo(raw) {
  const source = String(raw || '').trim();
  if (!source) {
    return { ok: false, error: 'Enter a shortcut first.' };
  }

  const modifiers = new Set();
  let key = null;

  for (const token of source.split('+')) {
    const trimmed = token.trim();
    if (!trimmed) {
      return { ok: false, error: 'That shortcut has an empty part. Check the plus signs.' };
    }

    const modifier = normalizeHotkeyModifier(trimmed);
    if (modifier) {
      if (key) return { ok: false, error: 'Modifiers must come before the main key.' };
      modifiers.add(modifier);
      continue;
    }

    if (key) {
      return { ok: false, error: 'A shortcut can only have one main key.' };
    }
    key = normalizeHotkeyKey(trimmed);
    if (!key) {
      return { ok: false, error: `“${trimmed}” isn’t a supported key.` };
    }
  }

  if (key === 'Fn') {
    if (modifiers.size) return { ok: false, error: 'Fn works on its own, without other keys.' };
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

  if (!modifiers.size) return { ok: false, error: 'Add a modifier like Cmd or Shift, or use Fn on its own.' };
  if (!key) return { ok: false, error: 'The shortcut is missing its main key.' };
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

/**
 * @param {KeyboardEvent} event
 * @returns {string | null}
 */
export function eventKeyToken(event) {
  const code = String(event?.code || '').trim();
  if (/^fn$/i.test(code)) return 'Fn';
  if (/^f19$/i.test(code)) return 'F19';
  return normalizeHotkeyKey(event.key);
}

/**
 * @param {KeyboardEvent} event
 * @returns {string | null}
 */
export function buildHotkeyFromEvent(event) {
  const key = eventKeyToken(event);
  // WKWebView often omits code=Fn for Globe/Fn; getModifierState still reports it.
  const fnPressed = key === 'Fn'
    || String(event.key || '').toLowerCase() === 'fn'
    || Boolean(event.getModifierState?.('Fn'));
  if (fnPressed && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
    return 'Fn';
  }
  if (!key || key === 'Fn') return null;

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

/**
 * @param {KeyboardEvent} event
 * @param {{ ok?: boolean, key?: string, requires?: object } | null} spec
 * @returns {boolean}
 */
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

/**
 * @returns {Array<{ label: string, value: string }>}
 */
export function getSuggestedHotkeyOptions() {
  const options = isFocusedMacDesktopMode()
    ? HOTKEY_PRESET_OPTIONS
    : HOTKEY_PRESET_OPTIONS.filter((option) => option.value !== 'Fn');
  return options.map((option) => ({
    value: option.value,
    label: option.value === 'Fn' ? 'Hold Fn' : humanizeHotkeyLabel(option.value)
  }));
}

/**
 * Shows a canonical shortcut the way this platform names its keys.
 * @param {unknown} raw
 * @returns {string}
 */
export function humanizeHotkeyLabel(raw) {
  const value = String(raw || '').trim();
  if (!value) return '';
  const mac = isMacPlatform();
  return value.split('+').map((token) => {
    if (token === 'CmdOrCtrl') return mac ? 'Cmd' : 'Ctrl';
    if (token === 'Alt') return mac ? 'Option' : 'Alt';
    return token;
  }).join('+');
}

/**
 * @param {unknown} arch
 * @returns {string}
 */
export function humanizeArchitecture(arch) {
  const value = String(arch || '').trim().toLowerCase();
  if (['aarch64', 'arm64'].includes(value)) return 'Apple silicon';
  if (['x86_64', 'x64', 'amd64'].includes(value)) return 'Intel';
  return arch || 'unknown arch';
}

/**
 * @param {object | null} [device]
 * @returns {string}
 */
export function describeMachineLabel(device = state.currentDeviceProfile) {
  if (!device) return 'This Mac';
  if (String(device.os || '').toLowerCase() === 'macos') {
    const arch = humanizeArchitecture(device.architecture);
    return arch === 'Apple silicon' ? 'This Apple silicon Mac' : (arch === 'Intel' ? 'This Intel Mac' : 'This Mac');
  }
  return 'This device';
}

/**
 * @param {unknown} [raw]
 * @returns {string}
 */
export function instructionHotkeyLabel(raw = state.savedDictationHotkey || state.pendingDictationHotkey || state.defaultDictationHotkey) {
  const value = String(raw || '').trim();
  if (!value) return 'your shortcut';
  return value === 'Fn' ? 'Fn / Globe' : humanizeHotkeyLabel(value);
}

/** @returns {boolean} */
export function isHoldToTalkHotkey() {
  return state.dictationTriggerMode === 'global-hold'
    || state.dictationTriggerMode === 'focused-window-hold'
    || state.activeHotkeySpec?.key === 'Fn';
}

/** @returns {string} */
export function idlePillMessage() {
  if (!isNativeDesktopMode()) return '';
  if (!isFocusedMacDesktopMode()) return 'Murmur dictates on macOS';
  if (!state.currentOnboarding) return 'Checking your setup…';
  if (!state.nativeDictationModelReady) return 'Finish setup in Murmur';
  if (!state.savedDictationHotkey) return 'Shortcut off · open Settings';
  if (state.dictationTriggerMode === 'focused-window-hold') {
    return `Focus Murmur, then hold ${instructionHotkeyLabel(state.savedDictationHotkey)}`;
  }
  if (isHoldToTalkHotkey()) {
    return `Hold ${instructionHotkeyLabel(state.savedDictationHotkey)} to dictate`;
  }
  return `Press ${instructionHotkeyLabel(state.savedDictationHotkey)} to dictate`;
}

/**
 * @param {string} trigger
 * @returns {string}
 */
export function listeningStatusForTrigger(trigger) {
  if (trigger === 'hotkey-hold' || isHoldToTalkHotkey()) {
    return `Listening… release ${instructionHotkeyLabel(state.savedDictationHotkey)} to transcribe.`;
  }
  if (trigger === 'hotkey') {
    return `Listening… press ${instructionHotkeyLabel(state.savedDictationHotkey)} again to transcribe.`;
  }
  return 'Listening… click Stop to transcribe.';
}

/**
 * @param {string} trigger
 * @returns {string}
 */
export function completedStatusForTrigger(trigger) {
  if (trigger === 'hotkey-hold' || trigger === 'hotkey' || isHoldToTalkHotkey()) {
    return 'Transcribed and added to your draft. Ready for the next one.';
  }
  return 'Transcribed and added to your draft.';
}
