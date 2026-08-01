/** Local Whisper model catalog UI and install/delete. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import { getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage } from '../platform.js';
import { modelDisplayName } from '../labels.js';
import { describeMachineLabel } from '../settings/hotkey-logic.js';
import {
  setUiMode, setStatus, setAppScreen, setSetupScreenMode, syncControls,
  setDictationModelStatus, setDictationModelBusy, refreshSelectedModelMeta
} from '../ui.js';
import { refreshDictationOnboarding } from './refresh.js';
import {
  getSelectedDictationModel,
  updateModelActionLabels
} from './model-selection.js';

export { getSelectedDictationModel, updateModelActionLabels };

/**
 * @param {object | null} device
 * @returns {string}
 */
export function describeDeviceProfile(device) {
  if (!device) return '';
  const ram = Number(device.total_memory_gb) || 0;
  const cores = Number(device.logical_cpu_cores) || 1;
  const machine = describeMachineLabel(device).replace(/^This /u, '');
  return `${machine} • ${ram} GB RAM • ${cores} logical CPU cores • ${device.os || 'unknown os'}`;
}

/**
 * @param {object} model
 * @returns {string}
 */
export function buildDictationModelLabel(model) {
  const fit = model.recommended
    ? 'Recommended'
    : (model.likely_runnable ? 'Likely runnable' : 'Heavy for this machine');
  const sizeValue = Number(model.approx_size_gb);
  const local = model.installed
    ? 'Installed'
    : (Number.isFinite(sizeValue) ? `${sizeValue} GB` : 'size unknown');
  return `${modelDisplayName(model)} • ${local} • ${fit}`;
}

/**
 * @param {Array<object>} models
 * @param {string} selectedModelId
 */
export function renderDictationModelOptions(models, selectedModelId) {
  if (!dom.dictationModelSelect) return;
  const safeModels = Array.isArray(models) ? models : [];
  state.dictationModels = safeModels;

  dom.dictationModelSelect.innerHTML = '';
  for (const model of safeModels) {
    const option = document.createElement('option');
    option.value = model.id;
    option.textContent = buildDictationModelLabel(model);
    dom.dictationModelSelect.appendChild(option);
  }

  if (!dom.dictationModelSelect.options.length) {
    dom.dictationModelSelect.value = '';
    updateModelActionLabels();
    refreshSelectedModelMeta();
    return;
  }

  const hasSelectedModel = Boolean(selectedModelId) && safeModels.some((model) => model.id === selectedModelId);
  if (hasSelectedModel) {
    dom.dictationModelSelect.value = selectedModelId;
    updateModelActionLabels();
    refreshSelectedModelMeta();
    return;
  }

  const installed = safeModels.find((model) => model.installed);
  const best = safeModels.find((model) => model.recommended || model.likely_runnable) || safeModels[0];
  dom.dictationModelSelect.value = installed?.id || best?.id || '';
  updateModelActionLabels();
  refreshSelectedModelMeta();
}

/** Downloads or switches the selected dictation model. */
export async function installSelectedDictationModel() {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  const selected = getSelectedDictationModel();
  if (!selected) {
    setStatus('Pick a speech model first.', 'error');
    return;
  }
  if (!state.whisperCliAvailable) {
    setStatus('whisper-cli is unavailable. In tauri:dev, click "Open CLI Setup (dev)", then "Refresh Setup".', 'error');
    return;
  }

  const prevNativeDictationModelReady = state.nativeDictationModelReady;
  let onboardingAfterInstall = null;

  try {
    state.isInstallingDictationModel = true;
    syncControls();
    setUiMode('loading');

    const isAlreadyInstalled = Boolean(selected.installed);
    if (isAlreadyInstalled) {
      setDictationModelBusy(`Switching active model to ${modelDisplayName(selected)}...`);
      setDictationModelStatus(`Switching to ${modelDisplayName(selected)}...`, 'neutral');
      setStatus(`Switching active model to ${modelDisplayName(selected)}...`, 'working');
    } else {
      const sizeValue = Number(selected.approx_size_gb);
      const sizeLabel = Number.isFinite(sizeValue) ? `~${sizeValue} GB` : 'size unknown';
      setDictationModelBusy(`Downloading ${modelDisplayName(selected)} (${sizeLabel}). Keep this window open...`);
      setDictationModelStatus(
        `Downloading ${modelDisplayName(selected)} (${sizeLabel}). Keep this window open while it downloads...`,
        'neutral'
      );
      setStatus(`Downloading ${modelDisplayName(selected)} model...`, 'working');
    }

    await tauriInvoke('install_dictation_model', { model: selected.id });
    const onboarding = await refreshDictationOnboarding({ quietStatus: true });
    onboardingAfterInstall = onboarding;

    if (!onboarding) {
      throw new Error('Model update completed, but setup refresh failed. Click Refresh Setup.');
    }
    if (!onboarding.selected_model_exists) {
      throw new Error('Model update finished, but selected model is not ready yet. Click Refresh Setup.');
    }
    if (!onboarding.whisper_cli_available) {
      throw new Error('Model is ready, but whisper-cli is unavailable. Click Refresh Setup.');
    }

    const selectedAfter = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
    setDictationModelBusy('');
    setUiMode('idle');
    setStatus(`Ready: ${modelDisplayName(selectedAfter) || modelDisplayName(selected)} is active for local dictation.`, 'ok');
    if (state.setupScreenMode === 'onboarding') {
      setAppScreen('dictation');
    }
  } catch (error) {
    const details = getErrorMessage(error);
    const modelNoLongerUsable = Boolean(onboardingAfterInstall)
      && (!onboardingAfterInstall.selected_model_exists || !onboardingAfterInstall.whisper_cli_available);
    state.nativeDictationModelReady = modelNoLongerUsable ? false : prevNativeDictationModelReady;
    setUiMode('error');
    setDictationModelBusy('');
    setDictationModelStatus(`Model update failed: ${details}`, 'error');
    setStatus(`Could not update model: ${details}`, 'error');
  } finally {
    state.isInstallingDictationModel = false;
    syncControls();
  }
}

/** Deletes the selected downloaded model after confirmation. */
export async function deleteSelectedDictationModel() {
  const tauriInvoke = getTauriInvoke();
  if (!isFocusedMacDesktopMode()) {
    setStatus('Model deletion is only available in macOS desktop mode.', 'error');
    return;
  }
  if (!tauriInvoke) {
    setStatus('Desktop bridge is not ready yet. Retry in a moment.', 'error');
    return;
  }

  const selected = getSelectedDictationModel();
  if (!selected) {
    setStatus('Pick a speech model first.', 'error');
    setDictationModelStatus('Pick a downloaded model before deleting.', 'error');
    return;
  }
  if (!selected.installed) {
    setStatus('Selected model is not downloaded.', 'error');
    setDictationModelStatus(`${modelDisplayName(selected)} is not downloaded, so there is nothing to delete.`, 'neutral');
    return;
  }

  if (typeof window.confirm !== 'function') {
    setStatus('Delete confirmation is unavailable in this runtime.', 'error');
    setDictationModelStatus('Delete confirmation is unavailable. Restart the app and try again.', 'error');
    return;
  }

  let confirmed = false;
  try {
    confirmed = window.confirm(`Delete ${modelDisplayName(selected)} from local storage?`);
  } catch (error) {
    const details = getErrorMessage(error);
    setStatus(`Could not open delete confirmation: ${details}`, 'error');
    setDictationModelStatus(`Could not open delete confirmation: ${details}`, 'error');
    return;
  }
  if (!confirmed) {
    setStatus(`Delete canceled for ${modelDisplayName(selected)}.`, 'neutral');
    return;
  }

  try {
    state.isDeletingDictationModel = true;
    syncControls();
    setUiMode('loading');
    setDictationModelBusy(`Deleting ${modelDisplayName(selected)} from local storage...`);
    setDictationModelStatus(`Deleting ${modelDisplayName(selected)} from local storage...`, 'neutral');
    setStatus(`Deleting ${modelDisplayName(selected)}...`, 'working');

    await tauriInvoke('delete_dictation_model', { model: selected.id });
    const onboarding = await refreshDictationOnboarding({ quietStatus: true });

    if (!onboarding) {
      throw new Error('Delete completed, but setup refresh failed. Click Refresh Setup.');
    }

    if (onboarding.selected_model_exists) {
      const selectedAfter = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
      setStatus(
        `Deleted ${modelDisplayName(selected)}. Active model: ${modelDisplayName(selectedAfter) || onboarding.selected_model_id}.`,
        'ok'
      );
      if (state.setupScreenMode === 'onboarding') {
        setAppScreen('dictation');
      }
    } else {
      setStatus(`Deleted ${modelDisplayName(selected)}. Download another model to continue local dictation.`, 'neutral');
      setSetupScreenMode('onboarding');
      setAppScreen('onboarding');
    }
    setDictationModelBusy('');
    setUiMode('idle');
  } catch (error) {
    const details = getErrorMessage(error);
    setUiMode('error');
    setDictationModelBusy('');
    setDictationModelStatus(`Delete failed: ${details}`, 'error');
    setStatus(`Could not delete model: ${details}`, 'error');
  } finally {
    state.isDeletingDictationModel = false;
    syncControls();
  }
}

/** Opens the whisper.cpp setup guide. */
export async function openWhisperSetupPage() {
  const tauriInvoke = getTauriInvoke();
  try {
    if (tauriInvoke) {
      await tauriInvoke('open_whisper_setup_page');
    } else {
      window.open('https://github.com/ggml-org/whisper.cpp#quick-start', '_blank', 'noopener,noreferrer');
    }
    setStatus('Opened whisper.cpp setup guide for tauri:dev troubleshooting.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    setStatus(`Could not open setup page: ${details}`, 'error');
  }
}
