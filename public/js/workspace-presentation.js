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
    authorized: 'Microphone · allowed', not_determined: 'Microphone · needs your permission',
    denied: 'Microphone · access denied', restricted: 'Microphone · restricted by policy',
    unavailable: 'Microphone · check unavailable', unknown: 'Microphone · access not verified'
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
    dom.requiredMicrophoneAction.textContent = state.isRequestingMicrophone ? 'Waiting for macOS…' : action === 'allow' ? 'Allow microphone' : action === 'settings' ? 'Open Microphone settings' : 'Recheck access';
    dom.requiredMicrophoneAction.disabled = state.isRequestingMicrophone;
  }
  if (dom.runtimeNote) dom.runtimeNote.textContent = native
    ? 'On your Mac. Your voice stays here. Transcribed locally with Whisper.'
    : 'Browser mode · Speech recognition may use your browser’s online service. On-device processing is not guaranteed. You can always type here.';
  if (dom.captureGuidance) dom.captureGuidance.textContent = !native
    ? (SpeechRecognitionApi ? 'Browser speech may use an online service. Start, speak, then stop. Or simply type below.' : 'Speech recognition isn’t available in this browser. You can type, edit, and copy your draft.')
    : !isFocusedMacDesktopMode() ? 'Native dictation is available on macOS. You can still type and copy a draft.'
    : ready ? (!state.savedDictationHotkey ? 'Start with the button. Speak naturally, then stop to transcribe.'
      : isHoldToTalkHotkey() ? `Start with the button, or hold ${instructionHotkeyLabel(state.savedDictationHotkey)} and release to transcribe.`
      : `Start with the button, or press ${instructionHotkeyLabel(state.savedDictationHotkey)} to start and stop.`)
    : !state.whisperCliAvailable ? 'The local speech engine needs attention. Open Setup & settings to recheck it. You can still type a draft.'
    : !state.nativeDictationModelReady ? 'Download a speech model in Setup & settings to start dictating. You can still type a draft.'
    : 'Allow microphone access in Setup & settings to start dictating. Your draft is ready to use.';
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
    : 'Local engine details will appear after a successful check.';
}
