/** Required setup and runtime context, shared by both screens without navigation side effects. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { isNativeDesktopMode, isFocusedMacDesktopMode } from './platform.js';
import { SpeechRecognitionApi } from './constants.js';
import { isNativeSetupReady, microphoneRecoveryAction } from './readiness.js';
import { syncSetupSections } from './navigation.js';
import { instructionHotkeyLabel, isHoldToTalkHotkey } from './settings/hotkey-logic.js';

const MICROPHONE_COPY = {
  authorized: 'Microphone · allowed', not_determined: 'Microphone · not allowed yet',
  denied: 'Microphone · access denied', restricted: 'Microphone · restricted by policy',
  unavailable: 'Microphone · couldn’t check', unknown: 'Microphone · not checked yet'
};

/** One short line under the capture buttons, only when there is something the buttons don't already say. */
function captureGuidance(native, active) {
  if (!native) return SpeechRecognitionApi ? '' : 'This browser can’t recognize speech. You can still type, edit, and copy.';
  if (!isFocusedMacDesktopMode()) return 'Dictation needs macOS. You can still type and copy here.';
  if (active) return '';
  if (isNativeSetupReady()) {
    if (!state.savedDictationHotkey) return '';
    const shortcut = instructionHotkeyLabel(state.savedDictationHotkey);
    return isHoldToTalkHotkey() ? `Or hold ${shortcut} and release to transcribe.` : `Or press ${shortcut} to start and stop.`;
  }
  if (!state.whisperCliAvailable) return 'The speech engine needs attention. See Settings.';
  if (!state.nativeDictationModelReady) return 'Download a speech model in Settings to start.';
  return 'Allow microphone access in Settings to start.';
}

export function renderWorkspaceReadiness() {
  const native = isNativeDesktopMode();
  const active = state.isDictating || state.isStartingDictation || state.nativeStopRequestInFlight;
  if (dom.startDictationBtn) dom.startDictationBtn.hidden = active;
  if (dom.stopDictationBtn) dom.stopDictationBtn.hidden = !active;
  const microphone = state.macosPermissions?.microphone || 'unknown';
  syncSetupSections();
  if (dom.microphoneHealthEl) {
    dom.microphoneHealthEl.textContent = MICROPHONE_COPY[microphone];
    dom.microphoneHealthEl.dataset.state = microphone === 'authorized' ? 'ok' : 'pending';
  }
  if (dom.requiredMicrophoneAction) {
    const action = microphoneRecoveryAction(microphone);
    dom.requiredMicrophoneAction.hidden = microphone === 'authorized';
    dom.requiredMicrophoneAction.dataset.permissionAction = action;
    dom.requiredMicrophoneAction.dataset.permission = 'microphone';
    dom.requiredMicrophoneAction.textContent = state.isRequestingMicrophone ? 'Waiting for macOS…' : action === 'allow' ? 'Allow microphone' : action === 'settings' ? 'Open System Settings' : 'Recheck';
    dom.requiredMicrophoneAction.disabled = state.isRequestingMicrophone;
  }
  if (dom.runtimeNote) dom.runtimeNote.textContent = isFocusedMacDesktopMode()
    ? 'Transcribed on this Mac. Your voice never leaves it.'
    : native ? 'Native dictation is available on macOS.' : 'Browser mode: speech recognition may use an online service.';
  if (dom.captureGuidance) dom.captureGuidance.textContent = captureGuidance(native, active);
  if (dom.sessionStopBtn) {
    dom.sessionStopBtn.hidden = !state.isDictating && !state.isStartingDictation;
    dom.sessionStopBtn.disabled = Boolean(dom.stopDictationBtn?.disabled);
  }
  if (dom.setupTechnicalDetails) dom.setupTechnicalDetails.textContent = state.currentOnboarding
    ? `Engine: ${state.currentOnboarding.whisper_cli_path || 'unavailable'}\nModel: ${state.currentOnboarding.selected_model_path || 'not downloaded'}\n${dom.dictationDeviceProfileEl?.textContent || ''}`
    : 'Engine and model locations appear after a successful check.';
}
