/** Shared UI status, screens, and control sync. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { SpeechRecognitionApi, PILL_STATUS_EVENT } from './constants.js';
import {
  getTauriEventApi, isNativeDesktopMode, isFocusedMacDesktopMode
} from './platform.js';
import { updateDictationWaveform, resetDictationWaveform } from './waveform.js';
import { modelDisplayName } from './labels.js';
import { syncBackgroundUiControls } from './settings/background-ui-controls.js';
import { instructionHotkeyLabel, isHoldToTalkHotkey, idlePillMessage } from './settings/hotkey-logic.js';
import { syncDictationHotkeyUi } from './settings/hotkey-ui.js';
import { getSelectedDictationModel, updateModelActionLabels } from './onboarding/model-selection.js';

export function setUiMode(mode) {
  document.body.dataset.mode = mode;
  if (mode === 'listening') {
    updateDictationWaveform(state.liveAudioLevel, state.liveAudioBars, mode);
  } else {
    resetDictationWaveform(mode);
  }
}


export function setStatus(message, tone = 'neutral') {
  dom.statusEl.textContent = message;
  dom.statusEl.dataset.tone = tone;
  syncHotkeyPillForStatus(message, tone);
}


export function setHotkeyPill(message, pillState = 'idle', visible = true) {
  emitHotkeyPillOverlay(message, pillState, visible);
}


export function emitHotkeyPillOverlay(message, pillState = 'idle', visible = true) {
  const tauriEvent = getTauriEventApi();
  if (typeof tauriEvent?.emit !== 'function') return;

  tauriEvent.emit(PILL_STATUS_EVENT, {
    message: String(message || '').trim() || 'Hold fn to dictate',
    state: String(pillState || 'idle'),
    visible: Boolean(visible)
  }).catch(() => {});
}


export function summarizeHotkeyPillStatus(message, tone = 'neutral') {
  if (!isFocusedMacDesktopMode()) return 'Desktop MVP: macOS only';
  const normalized = String(message || '').toLowerCase();
  const hotkeyLabel = instructionHotkeyLabel(state.savedDictationHotkey);

  if (tone === 'live') {
    if (isHoldToTalkHotkey()) {
      return `Listening - release ${hotkeyLabel}`;
    }
    return `Listening - press ${hotkeyLabel} again`;
  }
  if (tone === 'working') {
    if (normalized.includes('transcrib')) return 'Transcribing...';
    if (normalized.includes('microphone') || normalized.includes('starting') || normalized.includes('opening')) {
      return 'Starting dictation...';
    }
    return 'Working...';
  }
  if (tone === 'ok') {
    if (normalized.includes('transcrib') || normalized.includes('captured') || normalized.includes('transcript')) {
      return 'Transcript ready';
    }
    return idlePillMessage();
  }
  if (tone === 'error') {
    return 'Dictation error - check status';
  }
  return idlePillMessage();
}


/**
 * SPA-side pill sync. On focused macOS desktop the native overlay is driven by
 * Rust (`emit_dictation_state` → pill sync); the SPA must not double-emit.
 * Everywhere else, hide any leftover in-app pill state.
 */
export function syncHotkeyPillForStatus(_message, _tone = 'neutral') {
  if (isFocusedMacDesktopMode()) return;
  setHotkeyPill('', 'idle', false);
}


export function setAppScreen(screen) {
  const next = screen === 'dictation' ? 'dictation' : 'onboarding';
  if (dom.onboardingScreen) dom.onboardingScreen.hidden = next !== 'onboarding';
  if (dom.dictationScreen) dom.dictationScreen.hidden = next !== 'dictation';
  if (dom.appShell) dom.appShell.dataset.screen = next;
  document.body.dataset.screen = next;
}


export function setSetupScreenMode(mode) {
  state.setupScreenMode = mode === 'settings' ? 'settings' : 'onboarding';

  const settingsMode = state.setupScreenMode === 'settings';
  if (dom.setupModeChipEl) dom.setupModeChipEl.textContent = settingsMode ? 'SETTINGS' : 'ONBOARDING';
  if (dom.setupTitleEl) dom.setupTitleEl.textContent = settingsMode ? 'Manage local speech setup' : 'Set up local speech-to-text';
  if (dom.setupLeadEl) {
    dom.setupLeadEl.textContent = settingsMode
      ? 'Switch models, delete downloads, or re-check whisper-cli. Changes apply to this device only.'
      : 'Everything runs on-device. Pick a model, download it once, and this machine is ready.';
  }
  if (dom.setupStepsEl) dom.setupStepsEl.hidden = settingsMode;
}


export function syncFlowForSetupReadiness() {
  const setupReady = !isFocusedMacDesktopMode() || state.nativeDictationModelReady;
  if (!setupReady) {
    setSetupScreenMode('onboarding');
    setAppScreen('onboarding');
    return;
  }
  if (state.setupScreenMode === 'onboarding') {
    setAppScreen('dictation');
  }
}


export function setDictationModelStatus(message, tone = 'neutral') {
  if (!dom.dictationModelStatusEl) return;
  dom.dictationModelStatusEl.textContent = message;
  dom.dictationModelStatusEl.dataset.tone = tone;
}


export function setDictationModelBusy(message = '') {
  if (!dom.dictationModelBusyEl) return;
  const trimmed = String(message || '').trim();
  dom.dictationModelBusyEl.hidden = !trimmed;
  dom.dictationModelBusyEl.textContent = trimmed;
}


export function setHealthPill(el, state, message) {
  if (!el) return;
  el.dataset.state = state;
  el.textContent = message;
}


export function syncSetupHealthPills() {
  const modelExists = Boolean(state.currentOnboarding?.selected_model_exists);

  if (!isNativeDesktopMode()) {
    setHealthPill(dom.whisperCliHealthEl, 'ok', 'whisper-cli: n/a (web)');
    setHealthPill(dom.dictationModelHealthEl, 'ok', 'model: n/a (web)');
    return;
  }
  if (!isFocusedMacDesktopMode()) {
    setHealthPill(dom.whisperCliHealthEl, 'error', 'whisper-cli: unsupported on this desktop OS');
    setHealthPill(dom.dictationModelHealthEl, 'error', 'model: unsupported on this desktop OS');
    return;
  }

  if (!state.currentOnboarding) {
    setHealthPill(dom.whisperCliHealthEl, 'pending', 'whisper-cli: checking');
    setHealthPill(dom.dictationModelHealthEl, 'pending', 'model: checking');
    return;
  }

  if (state.whisperCliAvailable) {
    setHealthPill(dom.whisperCliHealthEl, 'ok', 'whisper-cli: ready');
  } else {
    setHealthPill(dom.whisperCliHealthEl, 'error', 'whisper-cli: unavailable');
  }

  if (state.isInstallingDictationModel) {
    setHealthPill(dom.dictationModelHealthEl, 'working', 'model: downloading');
  } else if (state.isDeletingDictationModel) {
    setHealthPill(dom.dictationModelHealthEl, 'working', 'model: deleting');
  } else if (modelExists) {
    setHealthPill(dom.dictationModelHealthEl, 'ok', 'model: ready');
  } else {
    setHealthPill(dom.dictationModelHealthEl, 'pending', 'model: required');
  }
}


export function refreshSelectedModelMeta() {
  if (!dom.dictationModelMetaEl) return;
  const selected = getSelectedDictationModel();
  if (!selected) {
    dom.dictationModelMetaEl.textContent = 'Pick a model to view speed, quality, and local install state.';
    return;
  }

  const sizeValue = Number(selected.approx_size_gb);
  const sizeLabel = Number.isFinite(sizeValue)
    ? `${sizeValue.toFixed(2).replace(/\.00$/u, '')} GB`
    : 'size unknown';

  const parts = [
    modelDisplayName(selected),
    sizeLabel,
    selected.speed_note || 'speed unknown',
    selected.quality_note || 'quality unknown',
    selected.installed ? 'downloaded locally' : 'not downloaded',
    selected.recommended ? 'recommended for this machine' : (selected.likely_runnable ? 'fits this machine' : 'likely heavy on this machine')
  ];
  dom.dictationModelMetaEl.textContent = parts.join(' • ');
}


export function syncControls() {
  const hasCaptureSupport = isFocusedMacDesktopMode() || (!isNativeDesktopMode() && Boolean(SpeechRecognitionApi));
  const dictationModelMissing = isFocusedMacDesktopMode() && !state.nativeDictationModelReady;
  const lockControls = state.isInstallingDictationModel
    || state.isDeletingDictationModel
    || state.isSavingInputDevice
    || state.isSavingBackgroundUiSettings;
  const hotkeyDisabled = lockControls || !isNativeDesktopMode();
  const selected = getSelectedDictationModel();
  const setupReady = !isFocusedMacDesktopMode() || state.nativeDictationModelReady;
  const selectedAlreadyActive = Boolean(selected?.installed)
    && Boolean(state.currentOnboarding?.selected_model_exists)
    && state.currentOnboarding?.selected_model_id === selected.id;
  const normalizedPending = String(state.pendingDictationHotkey || '').trim();
  const normalizedSaved = String(state.savedDictationHotkey || '').trim();
  const pendingMatchesSaved = normalizedPending === normalizedSaved;
  const hasPendingHotkey = Boolean(normalizedPending);
  const savedIsDefault = Boolean(normalizedSaved && state.defaultDictationHotkey && normalizedSaved === state.defaultDictationHotkey);

  dom.startDictationBtn.disabled = (
    lockControls
    || !hasCaptureSupport
    || state.isDictating
    || state.isStartingDictation
    || state.nativeStopRequestInFlight
    || dictationModelMissing
  );
  dom.stopDictationBtn.disabled = (
    lockControls
    || !hasCaptureSupport
    || state.nativeStopRequestInFlight
    || (!state.isDictating && !state.isStartingDictation)
  );
  dom.clearTranscriptBtn.disabled = lockControls || state.nativeStopRequestInFlight;

  if (dom.installDictationModelBtn) {
    dom.installDictationModelBtn.disabled = (
      lockControls
      || !dom.dictationModelSelect?.value
      || !state.whisperCliAvailable
      || selectedAlreadyActive
    );
  }
  if (dom.deleteDictationModelBtn) {
    dom.deleteDictationModelBtn.disabled = lockControls || !selected?.installed;
  }
  if (dom.openWhisperSetupBtn) {
    dom.openWhisperSetupBtn.disabled = lockControls;
  }
  if (dom.retryWhisperCheckBtn) {
    dom.retryWhisperCheckBtn.disabled = lockControls;
  }
  if (dom.dictationModelSelect) {
    dom.dictationModelSelect.disabled = lockControls;
  }
  if (dom.dictationInputSelectEl) {
    dom.dictationInputSelectEl.disabled = lockControls || !isNativeDesktopMode();
  }
  if (dom.onboardingContinueBtn) {
    dom.onboardingContinueBtn.disabled = lockControls || (state.setupScreenMode === 'onboarding' && !setupReady);
    dom.onboardingContinueBtn.textContent = state.setupScreenMode === 'settings'
      ? 'Done'
      : (setupReady ? 'Start Dictation' : 'Complete Setup to Continue');
  }
  if (dom.openSettingsBtn) {
    dom.openSettingsBtn.disabled = lockControls || !isFocusedMacDesktopMode();
  }
  if (dom.recordDictationHotkeyBtn) {
    dom.recordDictationHotkeyBtn.disabled = hotkeyDisabled;
  }
  if (dom.saveDictationHotkeyBtn) {
    dom.saveDictationHotkeyBtn.disabled = hotkeyDisabled || !hasPendingHotkey || pendingMatchesSaved || state.isCapturingDictationHotkey;
  }
  if (dom.resetDictationHotkeyBtn) {
    dom.resetDictationHotkeyBtn.disabled = hotkeyDisabled || savedIsDefault;
  }
  if (dom.clearDictationHotkeyBtn) {
    dom.clearDictationHotkeyBtn.disabled = hotkeyDisabled || !normalizedSaved;
  }
  if (dom.focusedFieldInsertToggleEl) {
    dom.focusedFieldInsertToggleEl.disabled = hotkeyDisabled || state.isSavingFocusedFieldInsertSetting;
    dom.focusedFieldInsertToggleEl.checked = state.focusedFieldInsertEnabled;
  }

  if (dom.appShell) {
    dom.appShell.setAttribute('aria-busy', lockControls ? 'true' : 'false');
  }
  if (dom.quickDictationFab) {
    if (isNativeDesktopMode()) {
      dom.quickDictationFab.hidden = true; // pill window handles this in native desktop mode
    } else {
      dom.quickDictationFab.disabled = dom.startDictationBtn.disabled && dom.stopDictationBtn.disabled;
      dom.quickDictationFab.setAttribute('aria-label',
        state.isDictating || state.isStartingDictation ? 'Stop dictation' : 'Start dictation');
    }
  }
  syncDictationHotkeyUi();
  syncBackgroundUiControls();
  updateModelActionLabels();
  syncSetupHealthPills();
}


export function setDictationState(dictating) {
  state.isDictating = dictating;
  syncControls();
}

