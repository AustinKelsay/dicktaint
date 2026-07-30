/** Mutable application state for the dictation SPA.
 *
 * Fields live in logical slices (`webSpeech`, `nativeSession`, `onboarding`,
 * `settingsPrefs`, `uiBusy`) plus top-level transcript draft/history.
 * Flat property names remain enumerable aliases onto those slices so callers
 * can keep using `state.isDictating` etc. without behavior change.
 */
import {
  DEFAULT_DICTATION_HOTKEY,
  DEFAULT_PILL_VISIBILITY_MODE,
  DEFAULT_MENU_BAR_MODE,
  DEFAULT_CLOSE_ACTION
} from './constants.js';

/**
 * Define enumerable get/set aliases on `root` that read/write `root[slice][key]`.
 * @param {Record<string, any>} root
 * @param {string} slice
 * @param {string[]} keys
 */
function aliasSliceFields(root, slice, keys) {
  for (const key of keys) {
    Object.defineProperty(root, key, {
      enumerable: true,
      configurable: true,
      get() {
        return this[slice][key];
      },
      set(value) {
        this[slice][key] = value;
      }
    });
  }
}

const webSpeech = {
  recognition: null,
  restartTimer: null,
  shouldKeepDictating: false,
  hasMicrophoneAccess: false
};

const nativeSession = {
  lastHotkeyToggleAtMs: 0,
  nativeHotkeyActionInFlight: false,
  nativeFnHoldActive: false,
  nativeFnStopRequested: false,
  nativeStopRequestInFlight: false,
  pendingNativeStartAfterStop: false,
  pendingNativeStartTrigger: null,
  activeNativeSessionId: null,
  nativeSessionIdToIgnore: null,
  rejectNextNativeAppend: false,
  committedNativeSessionIds: new Set(),
  startNativeDesktopDictationOverride: null
};

const onboarding = {
  isInstallingDictationModel: false,
  isDeletingDictationModel: false,
  currentDeviceProfile: null,
  nativeDictationModelReady: true,
  whisperCliAvailable: true,
  dictationModels: [],
  currentOnboarding: null,
  setupScreenMode: 'onboarding'
};

const settingsPrefs = {
  savedDictationHotkey: null,
  defaultDictationHotkey: DEFAULT_DICTATION_HOTKEY,
  preferredInputDevice: null,
  activeHotkeySpec: null,
  pendingDictationHotkey: '',
  isCapturingDictationHotkey: false,
  dictationTriggerMode: 'disabled',
  dictationTriggerStatus: 'Hotkey disabled.',
  dictationTriggerPermissionHint: '',
  focusedFieldInsertEnabled: false,
  focusedFieldInsertPermissionGranted: false,
  focusedFieldInsertPermissionStatus: 'Focused-field insertion is disabled.',
  pillVisibilityMode: DEFAULT_PILL_VISIBILITY_MODE,
  menuBarMode: DEFAULT_MENU_BAR_MODE,
  closeAction: DEFAULT_CLOSE_ACTION,
  isSavingBackgroundUiSettings: false,
  isSavingFocusedFieldInsertSetting: false,
  isSavingInputDevice: false
};

const uiBusy = {
  isDictating: false,
  isStartingDictation: false,
  liveAudioLevel: 0,
  liveAudioBars: []
};

export const state = {
  webSpeech,
  nativeSession,
  onboarding,
  settingsPrefs,
  uiBusy,
  currentDraftText: '',
  dictationHistory: [],
  dictationHistorySeq: 0
};

aliasSliceFields(state, 'webSpeech', [
  'recognition',
  'restartTimer',
  'shouldKeepDictating',
  'hasMicrophoneAccess'
]);

aliasSliceFields(state, 'nativeSession', [
  'lastHotkeyToggleAtMs',
  'nativeHotkeyActionInFlight',
  'nativeFnHoldActive',
  'nativeFnStopRequested',
  'nativeStopRequestInFlight',
  'pendingNativeStartAfterStop',
  'pendingNativeStartTrigger',
  'activeNativeSessionId',
  'nativeSessionIdToIgnore',
  'rejectNextNativeAppend',
  'committedNativeSessionIds',
  'startNativeDesktopDictationOverride'
]);

aliasSliceFields(state, 'onboarding', [
  'isInstallingDictationModel',
  'isDeletingDictationModel',
  'currentDeviceProfile',
  'nativeDictationModelReady',
  'whisperCliAvailable',
  'dictationModels',
  'currentOnboarding',
  'setupScreenMode'
]);

aliasSliceFields(state, 'settingsPrefs', [
  'savedDictationHotkey',
  'defaultDictationHotkey',
  'preferredInputDevice',
  'activeHotkeySpec',
  'pendingDictationHotkey',
  'isCapturingDictationHotkey',
  'dictationTriggerMode',
  'dictationTriggerStatus',
  'dictationTriggerPermissionHint',
  'focusedFieldInsertEnabled',
  'focusedFieldInsertPermissionGranted',
  'focusedFieldInsertPermissionStatus',
  'pillVisibilityMode',
  'menuBarMode',
  'closeAction',
  'isSavingBackgroundUiSettings',
  'isSavingFocusedFieldInsertSetting',
  'isSavingInputDevice'
]);

aliasSliceFields(state, 'uiBusy', [
  'isDictating',
  'isStartingDictation',
  'liveAudioLevel',
  'liveAudioBars'
]);
