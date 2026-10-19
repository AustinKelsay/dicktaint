/** Local Whisper model catalog UI and install/delete. */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import { getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage } from '../platform.js';
import { modelDisplayName } from '../labels.js';
import { describeMachineLabel } from '../settings/hotkey-logic.js';
import {
  setUiMode, setStatus, syncControls,
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
  const os = String(device.os || '').toLowerCase();
  const osLabel = os === 'macos' ? 'macOS' : (os === 'windows' ? 'Windows' : (os === 'linux' ? 'Linux' : (device.os || 'unknown OS')));
  return `${machine} · ${ram} GB memory · ${cores} CPU cores · ${osLabel}`;
}

/**
 * @param {object} model
 * @returns {string}
 */
export function buildDictationModelLabel(model) {
  const fit = model.recommended
    ? 'Recommended'
    : (model.likely_runnable ? 'Fits this Mac' : 'May be slow here');
  const sizeValue = Number(model.approx_size_gb);
  const local = model.installed
    ? 'Downloaded'
    : (Number.isFinite(sizeValue) ? `${sizeValue} GB` : 'size unknown');
  return `${modelDisplayName(model)} · ${local} · ${fit}`;
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
    setStatus('Choose a speech model first.', 'error');
    return;
  }
  if (!state.whisperCliAvailable) {
    setStatus('The speech engine needs attention. Recheck, or quit and reopen Murmur.', 'error');
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
      setDictationModelBusy(`Switching to ${modelDisplayName(selected)}…`);
      setDictationModelStatus(`Switching to ${modelDisplayName(selected)}…`, 'neutral');
      setStatus(`Switching to ${modelDisplayName(selected)}…`, 'working');
    } else {
      const sizeValue = Number(selected.approx_size_gb);
      const sizeLabel = Number.isFinite(sizeValue) ? `about ${sizeValue} GB` : 'size unknown';
      setDictationModelBusy(`Downloading ${modelDisplayName(selected)} (${sizeLabel}). Keep Murmur open…`);
      setDictationModelStatus(
        `Downloading ${modelDisplayName(selected)} (${sizeLabel}). This happens once. Keep Murmur open until it finishes…`,
        'neutral'
      );
      setStatus(`Downloading ${modelDisplayName(selected)}…`, 'working');
    }

    await tauriInvoke('install_dictation_model', { model: selected.id });
    const onboarding = await refreshDictationOnboarding({ quietStatus: true });
    onboardingAfterInstall = onboarding;

    if (!onboarding) {
      throw new Error('The model updated, but the setup check failed. Click Recheck.');
    }
    if (!onboarding.selected_model_exists) {
      throw new Error('The model updated, but it isn’t ready yet. Click Recheck.');
    }
    if (!onboarding.whisper_cli_available) {
      throw new Error('The model is ready, but the speech engine is unavailable. Click Recheck.');
    }

    const selectedAfter = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
    setDictationModelBusy('');
    setUiMode('idle');
    setStatus(`${modelDisplayName(selectedAfter) || modelDisplayName(selected)} is ready. You can dictate now.`, 'ok');

  } catch (error) {
    const details = getErrorMessage(error);
    const modelNoLongerUsable = Boolean(onboardingAfterInstall)
      && (!onboardingAfterInstall.selected_model_exists || !onboardingAfterInstall.whisper_cli_available);
    state.nativeDictationModelReady = modelNoLongerUsable ? false : prevNativeDictationModelReady;
    setUiMode('error');
    setDictationModelBusy('');
    setDictationModelStatus(`Could not update the model: ${details}`, 'error');
    setStatus(`Could not update the model: ${details}`, 'error');
  } finally {
    state.isInstallingDictationModel = false;
    syncControls();
  }
}

/** Deletes the selected downloaded model after confirmation. */
export async function deleteSelectedDictationModel() {
  const tauriInvoke = getTauriInvoke();
  if (!isFocusedMacDesktopMode()) {
    setStatus('Models can only be removed in the Mac app.', 'error');
    return;
  }
  if (!tauriInvoke) {
    setStatus('Murmur is still starting. Try again in a moment.', 'error');
    return;
  }

  const selected = getSelectedDictationModel();
  if (!selected) {
    setStatus('Choose a speech model first.', 'error');
    setDictationModelStatus('Choose a downloaded model to remove.', 'error');
    return;
  }
  if (!selected.installed) {
    setStatus(`${modelDisplayName(selected)} isn’t downloaded.`, 'error');
    setDictationModelStatus(`${modelDisplayName(selected)} isn’t downloaded, so there’s nothing to remove.`, 'neutral');
    return;
  }

  if (typeof window.confirm !== 'function') {
    setStatus('Murmur couldn’t ask you to confirm. Quit and reopen, then try again.', 'error');
    setDictationModelStatus('Murmur couldn’t ask you to confirm. Quit and reopen, then try again.', 'error');
    return;
  }

  let confirmed = false;
  try {
    confirmed = window.confirm(`Remove ${modelDisplayName(selected)} from this Mac? You can download it again later.`);
  } catch (error) {
    const details = getErrorMessage(error);
    setStatus(`Could not ask you to confirm: ${details}`, 'error');
    setDictationModelStatus(`Could not ask you to confirm: ${details}`, 'error');
    return;
  }
  if (!confirmed) {
    setStatus(`Kept ${modelDisplayName(selected)}.`, 'neutral');
    return;
  }

  try {
    state.isDeletingDictationModel = true;
    syncControls();
    setUiMode('loading');
    setDictationModelBusy(`Removing ${modelDisplayName(selected)}…`);
    setDictationModelStatus(`Removing ${modelDisplayName(selected)}…`, 'neutral');
    setStatus(`Removing ${modelDisplayName(selected)}…`, 'working');

    await tauriInvoke('delete_dictation_model', { model: selected.id });
    const onboarding = await refreshDictationOnboarding({ quietStatus: true });

    if (!onboarding) {
      throw new Error('The model was removed, but the setup check failed. Click Recheck.');
    }

    if (onboarding.selected_model_exists) {
      const selectedAfter = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
      setStatus(
        `Removed ${modelDisplayName(selected)}. ${modelDisplayName(selectedAfter) || onboarding.selected_model_id} is now in use.`,
        'ok'
      );

    } else {
      setStatus(`Removed ${modelDisplayName(selected)}. Download a model to keep dictating.`, 'neutral');

    }
    setDictationModelBusy('');
    setUiMode('idle');
  } catch (error) {
    const details = getErrorMessage(error);
    setUiMode('error');
    setDictationModelBusy('');
    setDictationModelStatus(`Could not remove the model: ${details}`, 'error');
    setStatus(`Could not remove the model: ${details}`, 'error');
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
    setStatus('Opened the whisper.cpp setup guide.', 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    setStatus(`Could not open the setup guide: ${details}`, 'error');
  }
}
