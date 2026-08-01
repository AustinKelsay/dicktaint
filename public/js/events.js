/**
 * DOM and Tauri event wiring for dictation.
 *
 * Acyclic layering (leaves → orchestration):
 *   constants/state/dom/platform/labels/media-permissions/draft-transcript
 *   → hotkey-logic / background-ui-controls / model-selection / waveform
 *   → history | transcript | hotkey-ui | hotkeys | background-ui | models
 *   → ui | native-dictation | web-speech | onboarding
 *   → events (this file)
 */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import {
  DEFAULT_DICTATION_HOTKEY,
  DICTATION_HOTKEY_EVENT, DICTATION_STATE_EVENT, DICTATION_AUDIO_LEVEL_EVENT, SpeechRecognitionApi
} from './constants.js';
import { isNativeDesktopMode, isFocusedMacDesktopMode, shouldUseTauriCommands, getTauriInvoke, getErrorMessage } from './platform.js';
import {
  setUiMode, setStatus, setAppScreen, setSetupScreenMode, syncControls, setDictationState,
  setDictationModelStatus, refreshSelectedModelMeta
} from './ui.js';
import { modelDisplayName } from './labels.js';
import { renderDictationHistory, runDictationHistoryAction } from './history.js';
import { setDraftTranscriptText, appendTranscriptChunk } from './transcript.js';
import { clearRestartTimer, scheduleRecognitionRestart } from './speech-runtime.js';
import {
  applyNativeFnHoldState, handleNativeHoldKeydown, handleNativeHoldKeyup,
  handleNativeDictationStatePayload, handleNativeDictationAudioLevelPayload,
  startNativeDesktopDictation, stopNativeDesktopDictation, triggerDictationToggleFromHotkey
} from './native-dictation.js';
import {
  handleDictationHotkeyEvent, beginDictationHotkeyCapture, saveDictationHotkey, clearDictationHotkey
} from './settings/hotkeys.js';
import { setDictationHotkeyStatus } from './settings/hotkey-logic.js';
import {
  saveFocusedFieldInsertSetting
} from './settings/focused-field-insert.js';
import {
  saveMenuBarMode, saveCloseAction, savePillVisibilityMode
} from './settings/background-ui.js';
import { savePreferredInputDevice } from './settings/input-device.js';
import {
  installSelectedDictationModel, deleteSelectedDictationModel, openWhisperSetupPage,
  getSelectedDictationModel, updateModelActionLabels
} from './onboarding/models.js';
import { loadDictationOnboarding } from './onboarding/index.js';
import {
  describeSpeechError, isFatalSpeechError
} from './web-speech.js';
import { ensureMicrophoneAccess } from './media-permissions.js';

/** Wires Tauri dictation events and native hold/start/stop controls. */
export function initNativeListeners() {
  const tauriEventApi = window.__TAURI__?.event || null;
  if (isNativeDesktopMode() && tauriEventApi?.listen) {
    tauriEventApi.listen(DICTATION_HOTKEY_EVENT, ({ payload }) => {
      if (state.isCapturingDictationHotkey) return;
      if (state.dictationTriggerMode !== 'focused-window-hold') return;
      if (state.activeHotkeySpec?.ok && state.activeHotkeySpec.key === 'Fn') {
        applyNativeFnHoldState(payload?.pressed !== false);
        return;
      }
      if (payload?.pressed === false) return;
      triggerDictationToggleFromHotkey();
    }).catch(err => {
      console.error('Failed to register DICTATION_HOTKEY_EVENT listener', err);
      setStatus('Could not register dictation hotkey listener.', 'error');
    });

    tauriEventApi.listen(DICTATION_STATE_EVENT, ({ payload }) => {
      handleNativeDictationStatePayload(payload);
    }).catch(err => {
      console.error('Failed to register DICTATION_STATE_EVENT listener', err);
      setStatus('Could not register dictation state listener.', 'error');
    });

    tauriEventApi.listen(DICTATION_AUDIO_LEVEL_EVENT, ({ payload }) => {
      handleNativeDictationAudioLevelPayload(payload);
    }).catch(err => {
      console.error('Failed to register DICTATION_AUDIO_LEVEL_EVENT listener', err);
    });
  }

  document.addEventListener('keydown', handleDictationHotkeyEvent);
  document.addEventListener('keyup', handleDictationHotkeyEvent);

  if (!isFocusedMacDesktopMode()) return;

  dom.startDictationBtn.addEventListener('click', () => {
    startNativeDesktopDictation('button');
  });

  dom.stopDictationBtn.addEventListener('click', () => {
    void stopNativeDesktopDictation('button');
  });

  window.addEventListener('keydown', handleNativeHoldKeydown, true);
  window.addEventListener('keyup', handleNativeHoldKeyup, true);
}

/** Wires settings / onboarding UI handlers (macOS desktop only). */
export function initSettingsListeners() {
  if (!isFocusedMacDesktopMode()) return;

  if (dom.installDictationModelBtn) {
    dom.installDictationModelBtn.addEventListener('click', installSelectedDictationModel);
  }
  if (dom.deleteDictationModelBtn) {
    dom.deleteDictationModelBtn.addEventListener('click', deleteSelectedDictationModel);
  }
  if (dom.openWhisperSetupBtn) {
    dom.openWhisperSetupBtn.addEventListener('click', openWhisperSetupPage);
  }
  if (dom.retryWhisperCheckBtn) {
    dom.retryWhisperCheckBtn.addEventListener('click', () => {
      loadDictationOnboarding();
    });
  }
  if (dom.recordDictationHotkeyBtn) {
    dom.recordDictationHotkeyBtn.addEventListener('click', beginDictationHotkeyCapture);
  }
  if (dom.dictationHotkeyInputEl) {
    dom.dictationHotkeyInputEl.addEventListener('input', (event) => {
      state.pendingDictationHotkey = String(event?.currentTarget?.value || '').trim();
      state.isCapturingDictationHotkey = false;
      if (!state.pendingDictationHotkey) {
        setDictationHotkeyStatus(`Hotkey disabled. Default: ${state.defaultDictationHotkey || DEFAULT_DICTATION_HOTKEY}.`, 'neutral');
      } else {
        setDictationHotkeyStatus(`Pending hotkey: ${state.pendingDictationHotkey}. Click "Save Hotkey" to apply.`, 'neutral');
      }
      syncControls();
    });
  }
  if (dom.dictationHotkeyPresetsEl) {
    dom.dictationHotkeyPresetsEl.addEventListener('click', (event) => {
      const target = event.target instanceof Element
        ? event.target.closest('button[data-hotkey-preset]')
        : null;
      if (!target) return;

      state.pendingDictationHotkey = String(target.dataset.hotkeyPreset || '').trim();
      state.isCapturingDictationHotkey = false;
      if (dom.dictationHotkeyInputEl) {
        dom.dictationHotkeyInputEl.value = state.pendingDictationHotkey;
      }
      setDictationHotkeyStatus(`Preset selected: ${state.pendingDictationHotkey}. Click "Save Hotkey" to apply.`, 'neutral');
      syncControls();
    });
  }
  if (dom.saveDictationHotkeyBtn) {
    dom.saveDictationHotkeyBtn.addEventListener('click', async () => {
      const nextValue = String(dom.dictationHotkeyInputEl?.value || state.pendingDictationHotkey || '').trim();
      await saveDictationHotkey(nextValue);
    });
  }
  if (dom.resetDictationHotkeyBtn) {
    dom.resetDictationHotkeyBtn.addEventListener('click', async () => {
      state.pendingDictationHotkey = state.defaultDictationHotkey || DEFAULT_DICTATION_HOTKEY;
      await saveDictationHotkey(state.pendingDictationHotkey);
    });
  }
  if (dom.clearDictationHotkeyBtn) {
    dom.clearDictationHotkeyBtn.addEventListener('click', clearDictationHotkey);
  }
  if (dom.focusedFieldInsertToggleEl) {
    dom.focusedFieldInsertToggleEl.addEventListener('change', (event) => {
      const next = Boolean(event?.currentTarget?.checked);
      void saveFocusedFieldInsertSetting(next);
    });
  }
  if (dom.menuBarModeSelectEl) {
    dom.menuBarModeSelectEl.addEventListener('change', (event) => {
      const next = String(event?.currentTarget?.value || '').trim();
      void saveMenuBarMode(next);
    });
  }
  if (dom.closeActionSelectEl) {
    dom.closeActionSelectEl.addEventListener('change', (event) => {
      const next = String(event?.currentTarget?.value || '').trim();
      void saveCloseAction(next);
    });
  }
  if (dom.pillVisibilityModeSelectEl) {
    dom.pillVisibilityModeSelectEl.addEventListener('change', (event) => {
      const next = String(event?.currentTarget?.value || '').trim();
      void savePillVisibilityMode(next);
    });
  }
  if (dom.dictationInputSelectEl) {
    dom.dictationInputSelectEl.addEventListener('change', (event) => {
      const nextValue = String(event?.currentTarget?.value || '').trim();
      void savePreferredInputDevice(nextValue);
    });
  }
  if (dom.dictationModelSelect) {
    dom.dictationModelSelect.addEventListener('change', () => {
      const selected = getSelectedDictationModel();
      if (!selected) {
        setDictationModelStatus('Pick a model to manage download/use state.', 'neutral');
      } else if (selected.installed) {
        const isCurrent = Boolean(state.currentOnboarding?.selected_model_exists)
          && state.currentOnboarding?.selected_model_id === selected.id;
        setDictationModelStatus(
          isCurrent
            ? `${modelDisplayName(selected)} is active for dictation.`
            : `${modelDisplayName(selected)} is installed. Click "Use Installed" to switch.`,
          'neutral'
        );
      } else {
        setDictationModelStatus(
          `${modelDisplayName(selected)} is not downloaded yet. Click "Download + Use" to install it.`,
          'neutral'
        );
      }
      refreshSelectedModelMeta();
      updateModelActionLabels();
      syncControls();
    });
  }
}

/** Wires shared transcript / history / navigation controls. */
export function initSharedUiListeners() {
  dom.clearTranscriptBtn.addEventListener('click', () => {
    const tauriInvoke = shouldUseTauriCommands() ? getTauriInvoke() : null;
    if (tauriInvoke) {
      tauriInvoke('cancel_native_dictation').catch(() => {});
    }

    state.shouldKeepDictating = false;
    state.isStartingDictation = false;
    state.pendingNativeStartAfterStop = false;
    state.pendingNativeStartTrigger = null;
    state.nativeSessionIdToIgnore = state.activeNativeSessionId;
    state.rejectNextNativeAppend = false;
    state.activeNativeSessionId = null;
    setDictationState(false);
    syncControls();
    clearRestartTimer();
    setDraftTranscriptText('');
    setUiMode('idle');
    setStatus('Transcript cleared. Recent dictation history is still available in app state.', 'neutral');
  });

  if (dom.clearDictationHistoryBtn) {
    dom.clearDictationHistoryBtn.addEventListener('click', () => {
      state.dictationHistory = [];
      renderDictationHistory();
      setStatus('Recent dictation history cleared.', 'neutral');
    });
  }

  if (dom.dictationHistoryListEl) {
    dom.dictationHistoryListEl.addEventListener('click', (event) => {
      const target = event.target instanceof Element
        ? event.target.closest('button[data-history-action][data-history-id]')
        : null;
      if (!target) return;

      const historyAction = String(target.dataset.historyAction || '').trim();
      const historyId = String(target.dataset.historyId || '').trim();
      void runDictationHistoryAction(historyAction, historyId);
    });
  }

  if (dom.quickDictationFab) {
    dom.quickDictationFab.addEventListener('click', () => {
      if (state.isDictating || state.isStartingDictation) {
        if (!dom.stopDictationBtn.disabled) dom.stopDictationBtn.click();
      } else {
        if (!dom.startDictationBtn.disabled) dom.startDictationBtn.click();
      }
    });
  }

  dom.transcriptInput.addEventListener('input', () => {
    state.currentDraftText = dom.transcriptInput.value;
  });

  renderDictationHistory();

  if (dom.onboardingContinueBtn) {
    dom.onboardingContinueBtn.addEventListener('click', () => {
      if (state.setupScreenMode === 'onboarding' && isFocusedMacDesktopMode() && !state.nativeDictationModelReady) {
        setStatus('Complete setup first, then start dictation.', 'neutral');
        return;
      }
      setAppScreen('dictation');
      setStatus(state.setupScreenMode === 'settings' ? 'Settings closed.' : 'Dictation ready.', 'ok');
    });
  }

  if (dom.openSettingsBtn) {
    dom.openSettingsBtn.addEventListener('click', () => {
      setSetupScreenMode('settings');
      setAppScreen('onboarding');
      setStatus('Settings opened. Manage local model setup here.', 'neutral');
    });
  }
}

/** Wires browser SpeechRecognition start/stop (non-native path). */
export function initWebSpeechDictation() {
  if (!SpeechRecognitionApi) {
    syncControls();
    return;
  }

  state.recognition = new SpeechRecognitionApi();
  state.recognition.continuous = true;
  state.recognition.interimResults = true;
  state.recognition.lang = 'en-US';

  state.recognition.onstart = () => {
    state.isStartingDictation = false;
    setDictationState(true);
    setUiMode('listening');
    setStatus('Listening... speak now.', 'live');
  };

  state.recognition.onresult = (event) => {
    let interimTranscript = '';

    for (let i = event.resultIndex; i < event.results.length; i += 1) {
      const result = event.results[i];
      const chunk = result[0]?.transcript || '';
      if (result.isFinal) {
        appendTranscriptChunk(chunk, { source: 'web' });
      } else {
        interimTranscript += chunk;
      }
    }

    dom.transcriptInput.value = `${state.currentDraftText} ${interimTranscript}`.trim();
  };

  state.recognition.onerror = (event) => {
    const errorCode = event.error || '';
    const speechError = describeSpeechError(errorCode);
    setDictationState(false);
    state.isStartingDictation = false;
    syncControls();
    setUiMode('error');
    setStatus(`Dictation error: ${speechError}`, 'error');

    if (isFatalSpeechError(errorCode)) {
      state.shouldKeepDictating = false;
      clearRestartTimer();
    }
  };

  state.recognition.onend = () => {
    setDictationState(false);
    state.isStartingDictation = false;
    syncControls();

    if (state.shouldKeepDictating) {
      scheduleRecognitionRestart();
      return;
    }

    clearRestartTimer();
    setUiMode('idle');
    setStatus('Dictation stopped.', 'neutral');
  };

  dom.startDictationBtn.addEventListener('click', async () => {
    if (!state.recognition || state.isDictating || state.isStartingDictation) return;

    try {
      state.shouldKeepDictating = true;
      clearRestartTimer();
      state.isStartingDictation = true;
      syncControls();
      setUiMode('loading');
      setStatus('Requesting microphone access...', 'working');
      await ensureMicrophoneAccess();
      state.recognition.start();
      setStatus('Starting dictation...', 'working');
    } catch (error) {
      const details = getErrorMessage(error);
      state.hasMicrophoneAccess = false;
      state.shouldKeepDictating = false;
      state.isStartingDictation = false;
      setDictationState(false);
      setUiMode('error');
      setStatus(`Could not start dictation: ${details}`, 'error');
    }
  });

  dom.stopDictationBtn.addEventListener('click', () => {
    state.shouldKeepDictating = false;
    state.isStartingDictation = false;
    clearRestartTimer();
    if (!state.recognition) return;
    state.recognition.stop();
    setDictationState(false);
    setUiMode('idle');
    setStatus('Dictation stopped.', 'neutral');
  });
}

/** Orchestrates native / settings / web dictation event wiring. */
export function initDictation() {
  initSharedUiListeners();
  initNativeListeners();

  if (isFocusedMacDesktopMode()) {
    initSettingsListeners();
    syncControls();
    return;
  }

  if (isNativeDesktopMode() && !isFocusedMacDesktopMode()) {
    syncControls();
    return;
  }

  initWebSpeechDictation();
}
