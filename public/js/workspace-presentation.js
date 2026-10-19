/** Required setup and runtime context, shared by both screens without navigation side effects. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { isNativeDesktopMode, isFocusedMacDesktopMode } from './platform.js';
import { SpeechRecognitionApi } from './constants.js';
import { isNativeSetupReady, microphoneRecoveryAction } from './readiness.js';
import { modelDisplayName } from './labels.js';
import { instructionHotkeyLabel, isHoldToTalkHotkey } from './settings/hotkey-logic.js';

export function renderWorkspaceReadiness() {
  const native = isNativeDesktopMode();
  const active = state.isDictating || state.isStartingDictation || state.nativeStopRequestInFlight;
  if (dom.startDictationBtn) dom.startDictationBtn.hidden = active;
  if (dom.stopDictationBtn) dom.stopDictationBtn.hidden = !active;
  const ready = isNativeSetupReady();
  const microphone = state.macosPermissions?.microphone || 'unknown';
  const micCopy = {
    authorized: 'Microphone · allowed', not_determined: 'Microphone · not allowed yet',
    denied: 'Microphone · access denied', restricted: 'Microphone · restricted by policy',
    unavailable: 'Microphone · couldn’t check', unknown: 'Microphone · not checked yet'
  };
  if (dom.requiredSetup) dom.requiredSetup.hidden = !native;
  if (dom.microphoneHealthEl) {
    dom.microphoneHealthEl.textContent = micCopy[microphone];
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
  if (dom.runtimeNote) dom.runtimeNote.textContent = native
    ? 'Transcribed on this Mac. Your voice never leaves it.'
    : 'Browser mode · Speech recognition may use an online service from your browser. You can always type instead.';
  if (dom.sidebarNote) dom.sidebarNote.textContent = native ? 'Private by design' : 'Browser mode';
  if (dom.captureGuidance) dom.captureGuidance.textContent = !native
    ? (SpeechRecognitionApi ? 'Start, speak, then stop. Your browser does the speech recognition. Or just type below.' : 'Speech recognition isn’t available in this browser. You can type, edit, and copy your draft.')
    : !isFocusedMacDesktopMode() ? 'Native dictation is available on macOS. You can still type and copy a draft here.'
    : state.nativeStopRequestInFlight ? 'Transcribing… this only takes a moment.'
    : state.isStartingDictation && !state.isDictating ? 'Opening the microphone…'
    : state.isDictating ? (!state.savedDictationHotkey ? 'Speak naturally. Click Stop & transcribe when you’re done.'
      : isHoldToTalkHotkey() ? `Speak naturally. Release ${instructionHotkeyLabel(state.savedDictationHotkey)} or click Stop & transcribe when you’re done.`
      : `Speak naturally. Press ${instructionHotkeyLabel(state.savedDictationHotkey)} again or click Stop & transcribe when you’re done.`)
    : ready ? (!state.savedDictationHotkey ? 'Start with the button. Speak naturally, then stop to transcribe.'
      : isHoldToTalkHotkey() ? `Start with the button, or hold ${instructionHotkeyLabel(state.savedDictationHotkey)} and release to transcribe.`
      : `Start with the button, or press ${instructionHotkeyLabel(state.savedDictationHotkey)} to start and stop.`)
    : !state.whisperCliAvailable ? 'The speech engine needs attention. Open Settings to recheck it. You can still type a draft.'
    : !state.nativeDictationModelReady ? 'Download a speech model in Settings to start dictating. You can type in the meantime.'
    : 'Allow microphone access in Settings to start dictating. You can type in the meantime.';
  if (dom.sessionStopBtn) {
    dom.sessionStopBtn.hidden = !state.isDictating && !state.isStartingDictation;
    dom.sessionStopBtn.disabled = Boolean(dom.stopDictationBtn?.disabled);
  }
  const recommended = state.dictationModels.find(model => model.recommended);
  if (dom.modelRecommendation) dom.modelRecommendation.textContent = recommended
    ? `Recommended for your Mac: ${modelDisplayName(recommended)} · ${Number(recommended.approx_size_gb).toFixed(2).replace(/\.00$/u, '')} GB. Download once, use offline.`
    : 'Download one speech model to transcribe on your Mac. No account needed.';
  if (dom.setupTechnicalDetails) dom.setupTechnicalDetails.textContent = state.currentOnboarding
    ? `Engine: ${state.currentOnboarding.whisper_cli_path || 'unavailable'}\nModel: ${state.currentOnboarding.selected_model_path || 'not downloaded'}\n${dom.dictationDeviceProfileEl?.textContent || ''}`
    : 'Engine and model locations appear after a successful check.';
}
