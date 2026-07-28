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
import { applyDictationHotkeyPayload } from './settings/hotkeys.js';
import { applyBackgroundUiPreferencesPayload } from './settings/background-ui.js';
import { summarizeHotkeyPillStatus, syncControls } from './ui.js';
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
    closeAction: state.closeAction
  };
}

/** Resets mutable dictation state to a predictable baseline for isolated tests. */
export function resetDictationStateForTests() {
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
  state.focusedFieldInsertPermissionGranted = false;
  state.focusedFieldInsertPermissionStatus = 'Focused-field insertion is disabled.';
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
    summarizeHotkeyPillStatus,
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
      syncControls();
    },
    setStartNativeDesktopDictationOverride(fn) {
      state.startNativeDesktopDictationOverride = typeof fn === 'function' ? fn : null;
    }
  };
}
