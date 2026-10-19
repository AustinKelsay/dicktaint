import { isNativeSetupReady } from '../readiness.js';
import { refreshMacosPermissions } from '../permissions.js';
/** Onboarding payload hydration and setup flow. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import {
  DEFAULT_PILL_VISIBILITY_MODE, DEFAULT_MENU_BAR_MODE,
  DEFAULT_CLOSE_ACTION, MAC_DESKTOP_ONLY_MESSAGE, SpeechRecognitionApi
} from '../constants.js';
import {
  getTauriInvoke, detectDesktopOs, isNativeDesktopMode, isFocusedMacDesktopMode, getErrorMessage
} from '../platform.js';
import { applyBackgroundUiPreferencesPayload } from '../settings/background-ui-controls.js';
import { applyDictationHotkeyPayload } from '../settings/hotkeys.js';
import { applyFocusedFieldInsertPayload, setFocusedFieldInsertStatus } from '../settings/focused-field-insert.js';
import { renderInputDeviceOptions } from '../settings/input-device.js';
import {
  describeDeviceProfile, renderDictationModelOptions
} from './models.js';
import { modelDisplayName } from '../labels.js';
import {
  setStatus, setAppScreen, setSetupScreenMode, syncFlowForSetupReadiness,
  setDictationModelStatus, setDictationModelBusy, syncControls
} from '../ui.js';
import { syncDictationHotkeyUi } from '../settings/hotkey-ui.js';
import { registerDictationOnboardingLoader } from './refresh.js';

/**
 * Hides every setup/settings card shown during macOS onboarding.
 * Shared by bail-out branches so none drift (e.g. forgetting the hotkey card).
 */
function hideSetupCards() {
  if (dom.dictationModelCard) dom.dictationModelCard.hidden = true;
  if (dom.dictationHotkeyCardEl) dom.dictationHotkeyCardEl.hidden = true;
  if (dom.dictationInputCardEl) dom.dictationInputCardEl.hidden = true;
  if (dom.focusedFieldInsertCardEl) dom.focusedFieldInsertCardEl.hidden = true;
  if (dom.backgroundUiCardEl) dom.backgroundUiCardEl.hidden = true;
}

/**
 * Resets shared onboarding/setup state and hides setup cards on bail-out paths.
 * @param {{
 *   nativeDictationModelReady: boolean,
 *   whisperCliAvailable: boolean,
 *   currentDeviceProfile?: object | null,
 *   dictationTriggerStatus: string,
 *   focusedFieldInsertPermissionStatus: string,
 *   focusedFieldInsertStatusMessage: string,
 *   focusedFieldInsertStatusTone?: string,
 * }} options
 */
function resetSetupBailoutState({
  nativeDictationModelReady,
  whisperCliAvailable,
  currentDeviceProfile = null,
  dictationTriggerStatus,
  focusedFieldInsertPermissionStatus,
  focusedFieldInsertStatusMessage,
  focusedFieldInsertStatusTone = 'neutral'
}) {
  state.nativeDictationModelReady = nativeDictationModelReady;
  state.whisperCliAvailable = whisperCliAvailable;
  state.currentOnboarding = null;
  state.currentDeviceProfile = currentDeviceProfile;
  state.savedDictationHotkey = null;
  state.preferredInputDevice = null;
  state.pendingDictationHotkey = '';
  state.activeHotkeySpec = null;
  state.isCapturingDictationHotkey = false;
  state.dictationTriggerMode = 'disabled';
  state.dictationTriggerStatus = dictationTriggerStatus;
  state.dictationTriggerPermissionHint = '';
  state.focusedFieldInsertEnabled = false;
  state.focusedFieldInsertPermissionGranted = false;
  state.focusedFieldInsertPermissionStatus = focusedFieldInsertPermissionStatus;
  state.pillVisibilityMode = DEFAULT_PILL_VISIBILITY_MODE;
  state.menuBarMode = DEFAULT_MENU_BAR_MODE;
  state.closeAction = DEFAULT_CLOSE_ACTION;
  state.isSavingBackgroundUiSettings = false;
  state.isSavingFocusedFieldInsertSetting = false;
  setFocusedFieldInsertStatus(focusedFieldInsertStatusMessage, focusedFieldInsertStatusTone);
  hideSetupCards();
}

/**
 * Loads dictation onboarding/setup state from Tauri (or web bypass).
 * @param {{ quietStatus?: boolean }} [options]
 * @returns {Promise<object | null>}
 */
export async function loadDictationOnboarding({ quietStatus = false } = {}) {
  // Web/mobile bypass desktop onboarding gates and run with browser/manual input paths.
  if (!isNativeDesktopMode()) {
    resetSetupBailoutState({
      nativeDictationModelReady: true,
      whisperCliAvailable: true,
      dictationTriggerStatus: 'Shortcut off.',
      focusedFieldInsertPermissionStatus: 'Typing into other apps isn’t available in a browser.',
      focusedFieldInsertStatusMessage: 'Typing into other apps isn’t available in a browser.'
    });
    state.onboardingCheckComplete = true;
    syncFlowForSetupReadiness();
    syncDictationHotkeyUi();
    syncControls();
    if (!state.isDictating && !state.isStartingDictation) {
      document.body.dataset.mode = 'idle';
      if (!quietStatus) {
        setStatus(SpeechRecognitionApi
          ? 'Ready. Start, speak, then stop.'
          : 'Speech recognition isn’t available in this browser. You can type or paste a draft.', 'neutral');
      }
    }
    return null;
  }
  if (!isFocusedMacDesktopMode()) {
    resetSetupBailoutState({
      nativeDictationModelReady: false,
      whisperCliAvailable: false,
      currentDeviceProfile: { os: detectDesktopOs(), architecture: navigator.platform || '' },
      dictationTriggerStatus: 'Shortcuts aren’t available on this system.',
      focusedFieldInsertPermissionStatus: 'Typing into other apps isn’t available on this system.',
      focusedFieldInsertStatusMessage: 'Typing into other apps isn’t available on this system.'
    });
    state.onboardingCheckComplete = true;
    syncFlowForSetupReadiness();
    setDictationModelBusy('');
    setDictationModelStatus(MAC_DESKTOP_ONLY_MESSAGE, 'error');
    if (!quietStatus) {
      setStatus(MAC_DESKTOP_ONLY_MESSAGE, 'error');
    }
    syncControls();
    return null;
  }

  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke) {
    state.permissionCheckComplete = true;
    resetSetupBailoutState({
      nativeDictationModelReady: false,
      whisperCliAvailable: false,
      dictationTriggerStatus: 'Murmur is still starting.',
      focusedFieldInsertPermissionStatus: 'Typing into other apps becomes available once Murmur finishes starting.',
      focusedFieldInsertStatusMessage: 'Typing into other apps becomes available once Murmur finishes starting.'
    });
    state.onboardingCheckComplete = true;
    syncFlowForSetupReadiness();
    syncDictationHotkeyUi();
    syncControls();
    return null;
  }

  try {
    if (!quietStatus && !state.isDictating && !state.isStartingDictation) {
      setStatus('Checking your setup…', 'working');
    }
    const readinessStatusRevision = state.statusRevision;
    setDictationModelBusy('');

    const onboarding = await tauriInvoke('get_dictation_onboarding');
    state.currentOnboarding = onboarding;
    state.currentDeviceProfile = onboarding.device || null;
    state.preferredInputDevice = onboarding.preferred_input_device || null;
    state.whisperCliAvailable = Boolean(onboarding.whisper_cli_available);
    state.nativeDictationModelReady = Boolean(onboarding.selected_model_exists && state.whisperCliAvailable);

    if (dom.dictationModelCard) {
      dom.dictationModelCard.hidden = false;
    }
    if (dom.dictationHotkeyCardEl) {
      dom.dictationHotkeyCardEl.hidden = false;
    }
    if (dom.dictationInputCardEl) {
      dom.dictationInputCardEl.hidden = false;
    }
    if (dom.focusedFieldInsertCardEl) {
      dom.focusedFieldInsertCardEl.hidden = false;
    }
    if (dom.backgroundUiCardEl) {
      dom.backgroundUiCardEl.hidden = false;
    }
    if (dom.dictationDeviceProfileEl) {
      dom.dictationDeviceProfileEl.textContent = describeDeviceProfile(onboarding.device);
    }

    renderDictationModelOptions(onboarding.models, onboarding.selected_model_id);
    renderInputDeviceOptions(onboarding.available_input_devices, onboarding.preferred_input_device);
    applyDictationHotkeyPayload(onboarding);
    applyBackgroundUiPreferencesPayload(onboarding);
    applyFocusedFieldInsertPayload(onboarding);

    if (dom.openWhisperSetupBtn) {
      dom.openWhisperSetupBtn.hidden = Boolean(onboarding.whisper_cli_available);
    }
    if (dom.developerHintEl) {
      dom.developerHintEl.hidden = Boolean(onboarding.whisper_cli_available);
    }

    if (!onboarding.whisper_cli_available && !onboarding.selected_model_exists) {
      setDictationModelStatus(
        'Murmur couldn’t find its speech engine. Recheck, or quit and reopen Murmur.',
        'error'
      );
      state.nativeDictationModelReady = false;
    } else if (!onboarding.whisper_cli_available && onboarding.selected_model_exists) {
      setDictationModelStatus(
        'Your model is ready, but the speech engine needs attention. Recheck, or quit and reopen Murmur.',
        'neutral'
      );
    } else if (onboarding.selected_model_exists) {
      const selected = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
      setDictationModelStatus(
        `${modelDisplayName(selected) || onboarding.selected_model_id} is ready to use.`,
        'ok'
      );
    } else {
      // The checklist and status bar already name the next step.
      setDictationModelStatus('', 'neutral');
      if (!quietStatus && !state.isDictating && !state.isStartingDictation) {
        setStatus('Download a speech model to finish setup.', 'neutral');
      }
    }

    state.onboardingCheckComplete = true;
    await refreshMacosPermissions();
    if (!quietStatus && state.statusRevision === readinessStatusRevision && !state.isDictating && !state.isStartingDictation && state.nativeDictationModelReady) {
      setStatus(isNativeSetupReady() ? 'Ready to dictate.' : 'Allow microphone access in Settings to start dictating.', isNativeSetupReady() ? 'ok' : 'neutral');
    }
    syncFlowForSetupReadiness();
    syncControls();
    if (document.body.dataset.mode === 'loading' && !state.isDictating && !state.isStartingDictation && !state.nativeStopRequestInFlight) document.body.dataset.mode = 'idle';
    return onboarding;
  } catch (error) {
    resetSetupBailoutState({
      nativeDictationModelReady: false,
      whisperCliAvailable: false,
      dictationTriggerStatus: 'Could not load the shortcut.',
      focusedFieldInsertPermissionStatus: 'Typing into other apps becomes available once setup loads.',
      focusedFieldInsertStatusMessage: 'Typing into other apps becomes available once setup loads.'
    });
    state.onboardingCheckComplete = true;
    await refreshMacosPermissions();
    syncFlowForSetupReadiness();
    const details = getErrorMessage(error);
    setDictationModelStatus(`Could not read your setup: ${details}`, 'error');
    setDictationModelBusy('');
    if (!quietStatus) {
      setStatus(`Could not load your setup: ${details}`, 'error');
    }
    syncDictationHotkeyUi();
    syncControls();
    return null;
  }
}

registerDictationOnboardingLoader(loadDictationOnboarding);
