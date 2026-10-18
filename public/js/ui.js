export { setAppScreen, setSetupScreenMode, syncFlowForSetupReadiness } from './navigation.js';
import { renderWorkspaceReadiness } from './workspace-presentation.js';
import { isNativeSetupReady } from './readiness.js';
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
  if (dom.workspaceTitle) dom.workspaceTitle.textContent = mode === 'listening' ? 'Listening. Take your time.'
    : mode === 'loading' && state.nativeStopRequestInFlight ? 'Finding your words…'
    : mode === 'loading' && state.isStartingDictation ? 'Opening your microphone…'
    : mode === 'error' ? 'Let’s try again.' : 'Ready when you are.';
  renderWorkspaceReadiness();
  if (mode === 'listening') {
    updateDictationWaveform(state.liveAudioLevel, state.liveAudioBars, mode);
  } else {
    resetDictationWaveform(mode);
  }
}


export function setStatus(message, tone = 'neutral') {
  if (dom.statusEl) {
    dom.statusEl.textContent = message;
    dom.statusEl.dataset.tone = tone;
  }
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
    setHealthPill(dom.whisperCliHealthEl, 'ok', 'Speech engine · browser mode');
    setHealthPill(dom.dictationModelHealthEl, 'ok', 'Speech model · browser mode');
    return;
  }
  if (!isFocusedMacDesktopMode()) {
    setHealthPill(dom.whisperCliHealthEl, 'error', 'Speech engine · macOS required');
    setHealthPill(dom.dictationModelHealthEl, 'error', 'Speech model · macOS required');
    return;
  }

  if (!state.currentOnboarding) {
    setHealthPill(dom.whisperCliHealthEl, 'pending', 'Speech engine · checking');
    setHealthPill(dom.dictationModelHealthEl, 'pending', 'Speech model · checking');
    return;
  }

  if (state.whisperCliAvailable) {
    setHealthPill(dom.whisperCliHealthEl, 'ok', 'Speech engine · ready');
  } else {
    setHealthPill(dom.whisperCliHealthEl, 'error', 'Speech engine · needs attention');
  }

  if (state.isInstallingDictationModel) {
    setHealthPill(dom.dictationModelHealthEl, 'working', 'Speech model · downloading');
  } else if (state.isDeletingDictationModel) {
    setHealthPill(dom.dictationModelHealthEl, 'working', 'Speech model · removing');
  } else if (modelExists) {
    setHealthPill(dom.dictationModelHealthEl, 'ok', 'Speech model · ready');
  } else {
    setHealthPill(dom.dictationModelHealthEl, 'pending', 'Speech model · download needed');
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
  const dictationModelMissing = !isNativeSetupReady();
  const lockControls = state.isInstallingDictationModel
    || state.isDeletingDictationModel
    || state.isSavingInputDevice
    || state.isSavingBackgroundUiSettings;
  const hotkeyDisabled = lockControls || !isNativeDesktopMode();
  const selected = getSelectedDictationModel();
  const setupReady = isNativeSetupReady();
  const selectedAlreadyActive = Boolean(selected?.installed)
    && Boolean(state.currentOnboarding?.selected_model_exists)
    && state.currentOnboarding?.selected_model_id === selected.id;
  const normalizedPending = String(state.pendingDictationHotkey || '').trim();
  const normalizedSaved = String(state.savedDictationHotkey || '').trim();
  const pendingMatchesSaved = normalizedPending === normalizedSaved;
  const hasPendingHotkey = Boolean(normalizedPending);
  const savedIsDefault = Boolean(normalizedSaved && state.defaultDictationHotkey && normalizedSaved === state.defaultDictationHotkey);

  if (dom.startDictationBtn) {
    dom.startDictationBtn.disabled = (
      lockControls
      || !hasCaptureSupport
      || state.isDictating
      || state.isStartingDictation
      || state.nativeStopRequestInFlight
      || dictationModelMissing
    );
  }
  if (dom.stopDictationBtn) {
    dom.stopDictationBtn.disabled = (
      !hasCaptureSupport
      || state.nativeStopRequestInFlight
      || (!state.isDictating && !state.isStartingDictation)
    );
  }
  if (dom.clearTranscriptBtn) {
    dom.clearTranscriptBtn.disabled = lockControls || state.nativeStopRequestInFlight;
  }

  if (dom.installDictationModelBtn) {
    dom.installDictationModelBtn.disabled = (
      lockControls
      || state.isDictating || state.isStartingDictation || state.nativeStopRequestInFlight
      || !dom.dictationModelSelect?.value
      || !state.whisperCliAvailable
      || selectedAlreadyActive
    );
  }
  if (dom.deleteDictationModelBtn) {
    dom.deleteDictationModelBtn.disabled = lockControls || state.isDictating || state.isStartingDictation || state.nativeStopRequestInFlight || !selected?.installed;
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
    dom.onboardingContinueBtn.disabled = false;
    dom.onboardingContinueBtn.textContent = state.setupScreenMode === 'settings'
      ? 'Back to dictation'
      : (setupReady ? 'Try dictation' : 'Open draft while you set up');
  }
  if (dom.openSettingsBtn) {
    dom.openSettingsBtn.disabled = false;
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
      dom.quickDictationFab.disabled = Boolean(dom.startDictationBtn?.disabled && dom.stopDictationBtn?.disabled);
      dom.quickDictationFab.setAttribute('aria-label',
        state.isDictating || state.isStartingDictation ? 'Stop dictation' : 'Start dictation');
    }
  }
  syncDictationHotkeyUi();
  syncBackgroundUiControls();
  updateModelActionLabels();
  syncSetupHealthPills();
  renderWorkspaceReadiness();
}


export function setDictationState(dictating) {
  state.isDictating = dictating;
  syncControls();
}

