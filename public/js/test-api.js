/** Test-only hooks exposed when __DICKTAINT_EXPOSE_TEST_API__ is set. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import {
  DEFAULT_PILL_VISIBILITY_MODE,
  DEFAULT_MENU_BAR_MODE,
  DEFAULT_CLOSE_ACTION
} from './constants.js';
import { appendTranscriptChunk, setDraftTranscriptText } from './transcript.js';
import { runDictationHistoryAction } from './history.js';
import {
  queueNativeStartAfterCurrentStop,
  maybeStartQueuedNativeDictation
} from './native-dictation.js';
import { applyDictationHotkeyPayload, saveDictationHotkey, clearDictationHotkey } from './settings/hotkeys.js';
import { applyBackgroundUiPreferencesPayload } from './settings/background-ui.js';
import {
  applyFocusedFieldInsertPayload,
  saveFocusedFieldInsertSetting,
  maybeInsertTranscriptIntoFocusedField
} from './settings/focused-field-insert.js';
import {
  installSelectedDictationModel,
  deleteSelectedDictationModel
} from './onboarding/models.js';
import { isFatalSpeechError, describeSpeechError } from './web-speech.js';
import { scheduleRecognitionRestart, clearRestartTimer } from './speech-runtime.js';
import { summarizeHotkeyPillStatus, syncControls, setHotkeyPill, emitHotkeyPillOverlay } from './ui.js';
import {
  handleNativeDictationStatePayload,
  handleNativeDictationAudioLevelPayload
} from './native-dictation.js';
import { defaultLiveAudioBars, resetDictationWaveform } from './waveform.js';
import { renderDictationHistory } from './history.js';

/** @returns {Record<string, unknown>} Snapshot of dictation state for tests. */
export function getDictationTestState() {
  return {
    currentDraftText: state.currentDraftText,
    dictationHistory: state.dictationHistory.map((entry) => ({ ...entry })),
    liveAudioLevel: state.liveAudioLevel,
    liveAudioBars: [...state.liveAudioBars],
    waveformAudioState: dom.dictationWaveformEl?.dataset?.audioState || 'idle',
    pendingNativeStartAfterStop: state.pendingNativeStartAfterStop,
    pendingNativeStartTrigger: state.pendingNativeStartTrigger,
    nativeStopRequestInFlight: state.nativeStopRequestInFlight,
    activeNativeSessionId: state.activeNativeSessionId,
    isDictating: state.isDictating,
    isStartingDictation: state.isStartingDictation,
    dictationTriggerMode: state.dictationTriggerMode,
    dictationTriggerStatus: state.dictationTriggerStatus,
    savedDictationHotkey: state.savedDictationHotkey,
    pendingDictationHotkey: state.pendingDictationHotkey,
    pillVisibilityMode: state.pillVisibilityMode,
    menuBarMode: state.menuBarMode,
    closeAction: state.closeAction,
    focusedFieldInsertEnabled: state.focusedFieldInsertEnabled,
    focusedFieldInsertPermissionGranted: state.focusedFieldInsertPermissionGranted,
    focusedFieldInsertPermissionStatus: state.focusedFieldInsertPermissionStatus,
    whisperCliAvailable: state.whisperCliAvailable,
    nativeDictationModelReady: state.nativeDictationModelReady,
    hasRestartTimer: Boolean(state.restartTimer),
    shouldKeepDictating: state.shouldKeepDictating
  };
}

/** Resets mutable dictation state to a predictable baseline for isolated tests. */
export function resetDictationStateForTests() {
  clearRestartTimer();
  state.dictationHistory = [];
  state.dictationHistorySeq = 0;
  state.isDictating = false;
  state.isStartingDictation = false;
  state.shouldKeepDictating = false;
  state.nativeStopRequestInFlight = false;
  state.pendingNativeStartAfterStop = false;
  state.pendingNativeStartTrigger = null;
  state.activeNativeSessionId = null;
  state.nativeSessionIdToIgnore = null;
  state.rejectNextNativeAppend = false;
  state.committedNativeSessionIds = new Set();
  state.startNativeDesktopDictationOverride = null;
  state.dictationTriggerMode = 'disabled';
  state.dictationTriggerStatus = 'Hotkey disabled.';
  state.dictationTriggerPermissionHint = '';
  state.focusedFieldInsertEnabled = false;
  state.focusedFieldInsertPermissionGranted = false;
  state.focusedFieldInsertPermissionStatus = 'Focused-field insertion is disabled.';
  state.isSavingFocusedFieldInsertSetting = false;
  state.pillVisibilityMode = DEFAULT_PILL_VISIBILITY_MODE;
  state.menuBarMode = DEFAULT_MENU_BAR_MODE;
  state.closeAction = DEFAULT_CLOSE_ACTION;
  state.isSavingBackgroundUiSettings = false;
  state.savedDictationHotkey = null;
  state.pendingDictationHotkey = '';
  state.activeHotkeySpec = null;
  state.currentDeviceProfile = { os: 'macos', architecture: 'aarch64' };
  state.liveAudioLevel = 0;
  state.liveAudioBars = defaultLiveAudioBars();
  state.recognition = null;
  setDraftTranscriptText('');
  renderDictationHistory();
  resetDictationWaveform('idle');
  syncControls();
}

/** Builds the object assigned to globalThis.__DICKTAINT_TEST_API__. */
export function createTestApi() {
  return {
    appendTranscriptChunk,
    runDictationHistoryAction,
    queueNativeStartAfterCurrentStop,
    maybeStartQueuedNativeDictation,
    setDraftTranscriptText,
    applyDictationHotkeyPayload,
    applyBackgroundUiPreferencesPayload,
    applyFocusedFieldInsertPayload,
    saveDictationHotkey,
    clearDictationHotkey,
    saveFocusedFieldInsertSetting,
    maybeInsertTranscriptIntoFocusedField,
    installSelectedDictationModel,
    deleteSelectedDictationModel,
    isFatalSpeechError,
    describeSpeechError,
    scheduleRecognitionRestart,
    clearRestartTimer,
    summarizeHotkeyPillStatus,
    setHotkeyPill,
    emitHotkeyPillOverlay,
    handleNativeDictationStatePayload,
    handleNativeDictationAudioLevelPayload,
    getState: getDictationTestState,
    resetState: resetDictationStateForTests,
    setNativeFlags(next = {}) {
      if (typeof next.nativeStopRequestInFlight === 'boolean') {
        state.nativeStopRequestInFlight = next.nativeStopRequestInFlight;
      }
      if (typeof next.isDictating === 'boolean') state.isDictating = next.isDictating;
      if (typeof next.isStartingDictation === 'boolean') state.isStartingDictation = next.isStartingDictation;
      if (typeof next.pendingNativeStartAfterStop === 'boolean') {
        state.pendingNativeStartAfterStop = next.pendingNativeStartAfterStop;
      }
      if (typeof next.pendingNativeStartTrigger === 'string' || next.pendingNativeStartTrigger === null) {
        state.pendingNativeStartTrigger = next.pendingNativeStartTrigger;
      }
      if (typeof next.shouldKeepDictating === 'boolean') {
        state.shouldKeepDictating = next.shouldKeepDictating;
      }
      if (typeof next.focusedFieldInsertEnabled === 'boolean') {
        state.focusedFieldInsertEnabled = next.focusedFieldInsertEnabled;
      }
      if (typeof next.whisperCliAvailable === 'boolean') {
        state.whisperCliAvailable = next.whisperCliAvailable;
      }
      syncControls();
    },
    setStartNativeDesktopDictationOverride(fn) {
      state.startNativeDesktopDictationOverride = typeof fn === 'function' ? fn : null;
    },
    /**
     * Installs a mock SpeechRecognition instance used by restart scheduling tests.
     * @param {{ start?: () => void, stop?: () => void } | null} recognition
     */
    setRecognition(recognition) {
      state.recognition = recognition;
    }
  };
}
