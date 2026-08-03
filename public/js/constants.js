/** Shared constants for dictation frontend runtime. */
export const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition || null;
export const DEFAULT_DICTATION_HOTKEY = 'CmdOrCtrl+Shift+D';
export const HOTKEY_MODIFIER_ORDER = ['CmdOrCtrl', 'Cmd', 'Ctrl', 'Alt', 'Shift', 'Super'];
export const DICTATION_HOTKEY_EVENT = 'dictation:hotkey-triggered';
export const DICTATION_STATE_EVENT = 'dictation:state-changed';
export const DICTATION_AUDIO_LEVEL_EVENT = 'dictation:audio-level';
export const NATIVE_HOLD_HOTKEYS = new Set(['Fn', 'F19']);
export const MAC_DESKTOP_ONLY_MESSAGE = 'Desktop MVP currently supports macOS only. Current mobile focus is iPhone (iOS).';
export const PILL_STATUS_EVENT = 'dicktaint://pill-status';
export const DICTATION_HISTORY_LIMIT = 10;
export const DICTATION_WAVEFORM_BAR_COUNT = 12;
export const DEFAULT_PILL_VISIBILITY_MODE = 'active-only';
export const DEFAULT_MENU_BAR_MODE = 'always';
export const DEFAULT_CLOSE_ACTION = 'hide-to-tray';
export const HOTKEY_PRESET_OPTIONS = [
  { value: 'Fn', label: 'Hold Fn' },
  { value: 'CmdOrCtrl+Shift+D', label: 'Cmd/Ctrl+Shift+D' },
  { value: 'CmdOrCtrl+Alt+Space', label: 'Cmd/Ctrl+Alt+Space' }
];
