/** Onboarding payload hydration and setup flow. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import {
  DEFAULT_PILL_VISIBILITY_MODE, DEFAULT_MENU_BAR_MODE,
  DEFAULT_CLOSE_ACTION, MAC_DESKTOP_ONLY_MESSAGE
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
 * Loads dictation onboarding/setup state from Tauri (or web bypass).
 * @param {{ quietStatus?: boolean }} [options]
 * @returns {Promise<object | null>}
 */
export async function loadDictationOnboarding({ quietStatus = false } = {}) {
  // Web/mobile bypass desktop onboarding gates and run with browser/manual input paths.
  if (!isNativeDesktopMode()) {
    state.nativeDictationModelReady = true;
    state.whisperCliAvailable = true;
    state.currentOnboarding = null;
    state.currentDeviceProfile = null;
    state.savedDictationHotkey = null;
    state.preferredInputDevice = null;
    state.pendingDictationHotkey = '';
    state.activeHotkeySpec = null;
    state.isCapturingDictationHotkey = false;
    state.dictationTriggerMode = 'disabled';
    state.dictationTriggerStatus = 'Hotkey disabled.';
    state.dictationTriggerPermissionHint = '';
    state.focusedFieldInsertEnabled = false;
    state.focusedFieldInsertPermissionGranted = false;
    state.focusedFieldInsertPermissionStatus = 'Focused-field insertion is disabled.';
    state.pillVisibilityMode = DEFAULT_PILL_VISIBILITY_MODE;
    state.menuBarMode = DEFAULT_MENU_BAR_MODE;
    state.closeAction = DEFAULT_CLOSE_ACTION;
    state.isSavingBackgroundUiSettings = false;
    state.isSavingFocusedFieldInsertSetting = false;
    setFocusedFieldInsertStatus('Focused-field insertion is disabled.', 'neutral');
    setSetupScreenMode('onboarding');
    if (dom.dictationModelCard) {
      dom.dictationModelCard.hidden = true;
    }
    if (dom.dictationHotkeyCardEl) {
      dom.dictationHotkeyCardEl.hidden = true;
    }
    if (dom.dictationInputCardEl) {
      dom.dictationInputCardEl.hidden = true;
    }
    if (dom.focusedFieldInsertCardEl) {
      dom.focusedFieldInsertCardEl.hidden = true;
    }
    if (dom.backgroundUiCardEl) {
      dom.backgroundUiCardEl.hidden = true;
    }
    if (dom.openSettingsBtn) {
      dom.openSettingsBtn.hidden = true;
    }
    setAppScreen('dictation');
    syncDictationHotkeyUi();
    syncControls();
    return null;
  }
  if (!isFocusedMacDesktopMode()) {
    state.nativeDictationModelReady = false;
    state.whisperCliAvailable = false;
    state.currentOnboarding = null;
    state.currentDeviceProfile = { os: detectDesktopOs(), architecture: navigator.platform || '' };
    state.preferredInputDevice = null;
    state.focusedFieldInsertEnabled = false;
    state.focusedFieldInsertPermissionGranted = false;
    state.focusedFieldInsertPermissionStatus = 'Focused-field insertion is unavailable on this platform.';
    state.pillVisibilityMode = DEFAULT_PILL_VISIBILITY_MODE;
    state.menuBarMode = DEFAULT_MENU_BAR_MODE;
    state.closeAction = DEFAULT_CLOSE_ACTION;
    state.isSavingBackgroundUiSettings = false;
    state.isSavingFocusedFieldInsertSetting = false;
    state.dictationTriggerMode = 'disabled';
    state.dictationTriggerStatus = 'Hotkey unavailable on this platform.';
    state.dictationTriggerPermissionHint = '';
    setFocusedFieldInsertStatus('Focused-field insertion is unavailable on this platform.', 'neutral');
    setSetupScreenMode('onboarding');
    if (dom.dictationModelCard) {
      dom.dictationModelCard.hidden = true;
    }
    if (dom.openSettingsBtn) {
      dom.openSettingsBtn.hidden = true;
    }
    if (dom.dictationInputCardEl) {
      dom.dictationInputCardEl.hidden = true;
    }
    if (dom.focusedFieldInsertCardEl) {
      dom.focusedFieldInsertCardEl.hidden = true;
    }
    if (dom.backgroundUiCardEl) {
      dom.backgroundUiCardEl.hidden = true;
    }
    setAppScreen('onboarding');
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
    state.nativeDictationModelReady = false;
    state.whisperCliAvailable = false;
    state.currentOnboarding = null;
    state.currentDeviceProfile = null;
    state.savedDictationHotkey = null;
    state.preferredInputDevice = null;
    state.pendingDictationHotkey = '';
    state.activeHotkeySpec = null;
    state.isCapturingDictationHotkey = false;
    state.dictationTriggerMode = 'disabled';
    state.dictationTriggerStatus = 'Desktop bridge offline.';
    state.dictationTriggerPermissionHint = '';
    state.focusedFieldInsertEnabled = false;
    state.focusedFieldInsertPermissionGranted = false;
    state.focusedFieldInsertPermissionStatus = 'Focused-field insertion is unavailable while desktop bridge is offline.';
    state.pillVisibilityMode = DEFAULT_PILL_VISIBILITY_MODE;
    state.menuBarMode = DEFAULT_MENU_BAR_MODE;
    state.closeAction = DEFAULT_CLOSE_ACTION;
    state.isSavingBackgroundUiSettings = false;
    state.isSavingFocusedFieldInsertSetting = false;
    setFocusedFieldInsertStatus('Focused-field insertion is unavailable while desktop bridge is offline.', 'neutral');
    setSetupScreenMode('onboarding');
    if (dom.openSettingsBtn) {
      dom.openSettingsBtn.hidden = true;
    }
    if (dom.dictationHotkeyCardEl) {
      dom.dictationHotkeyCardEl.hidden = true;
    }
    if (dom.dictationInputCardEl) {
      dom.dictationInputCardEl.hidden = true;
    }
    if (dom.focusedFieldInsertCardEl) {
      dom.focusedFieldInsertCardEl.hidden = true;
    }
    if (dom.backgroundUiCardEl) {
      dom.backgroundUiCardEl.hidden = true;
    }
    setAppScreen('onboarding');
    syncDictationHotkeyUi();
    syncControls();
    return null;
  }

  try {
    if (!quietStatus) {
      setStatus('Checking local speech-to-text setup...', 'working');
    }
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
    if (dom.openSettingsBtn) {
      dom.openSettingsBtn.hidden = false;
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

    if (!onboarding.whisper_cli_available && !onboarding.selected_model_exists) {
      setDictationModelStatus(
        `whisper-cli is unavailable. Packaged builds should include it. In tauri:dev, click "Open CLI Setup (dev)", then "Refresh Setup". Checked: ${onboarding.whisper_cli_path}`,
        'error'
      );
      state.nativeDictationModelReady = false;
    } else if (!onboarding.whisper_cli_available && onboarding.selected_model_exists) {
      setDictationModelStatus(
        `Model is ready, but whisper-cli is unavailable. In tauri:dev, click "Open CLI Setup (dev)", then "Refresh Setup". Checked: ${onboarding.whisper_cli_path}`,
        'neutral'
      );
    } else if (onboarding.selected_model_exists) {
      const selected = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
      setDictationModelStatus(
        `Speech-to-text ready: ${modelDisplayName(selected) || onboarding.selected_model_id}.`,
        'ok'
      );
      if (!quietStatus) {
        setStatus('Local speech-to-text is ready on this device.', 'ok');
      }
    } else {
      setDictationModelStatus(
        'Choose a model and click "Download + Use" to enable local speech-to-text.',
        'neutral'
      );
      if (!quietStatus) {
        setStatus('Setup required: download a local speech model for this device.', 'neutral');
      }
    }

    syncFlowForSetupReadiness();
    syncControls();
    return onboarding;
  } catch (error) {
    state.nativeDictationModelReady = false;
    state.whisperCliAvailable = false;
    state.currentOnboarding = null;
    state.currentDeviceProfile = null;
    state.savedDictationHotkey = null;
    state.preferredInputDevice = null;
    state.pendingDictationHotkey = '';
    state.activeHotkeySpec = null;
    state.isCapturingDictationHotkey = false;
    state.dictationTriggerMode = 'disabled';
    state.dictationTriggerStatus = 'Could not load hotkey state.';
    state.dictationTriggerPermissionHint = '';
    state.focusedFieldInsertEnabled = false;
    state.focusedFieldInsertPermissionGranted = false;
    state.focusedFieldInsertPermissionStatus = 'Focused-field insertion is unavailable while setup is loading.';
    state.pillVisibilityMode = DEFAULT_PILL_VISIBILITY_MODE;
    state.menuBarMode = DEFAULT_MENU_BAR_MODE;
    state.closeAction = DEFAULT_CLOSE_ACTION;
    state.isSavingBackgroundUiSettings = false;
    state.isSavingFocusedFieldInsertSetting = false;
    setFocusedFieldInsertStatus('Focused-field insertion is unavailable while setup is loading.', 'neutral');
    setSetupScreenMode('onboarding');
    if (dom.openSettingsBtn) {
      dom.openSettingsBtn.hidden = true;
    }
    if (dom.dictationHotkeyCardEl) {
      dom.dictationHotkeyCardEl.hidden = true;
    }
    if (dom.dictationInputCardEl) {
      dom.dictationInputCardEl.hidden = true;
    }
    if (dom.focusedFieldInsertCardEl) {
      dom.focusedFieldInsertCardEl.hidden = true;
    }
    if (dom.backgroundUiCardEl) {
      dom.backgroundUiCardEl.hidden = true;
    }
    setAppScreen('onboarding');
    const details = getErrorMessage(error);
    setDictationModelStatus(`Could not read setup state: ${details}`, 'error');
    setDictationModelBusy('');
    if (!quietStatus) {
      setStatus(`Could not load setup state: ${details}`, 'error');
    }
    syncDictationHotkeyUi();
    syncControls();
    return null;
  }
}

registerDictationOnboardingLoader(loadDictationOnboarding);

