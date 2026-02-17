const statusEl = document.getElementById('status');
const onboardingScreen = document.getElementById('onboardingScreen');
const dictationScreen = document.getElementById('dictationScreen');
const onboardingContinueBtn = document.getElementById('onboardingContinue');
const openSettingsBtn = document.getElementById('openSettings');
const backToDictationBtn = document.getElementById('backToDictation');
const setupModeChipEl = document.getElementById('setupModeChip');
const setupTitleEl = document.getElementById('setupTitle');
const setupLeadEl = document.getElementById('setupLead');
const setupStepsEl = document.getElementById('setupSteps');
const startDictationBtn = document.getElementById('startDictation');
const stopDictationBtn = document.getElementById('stopDictation');
const clearTranscriptBtn = document.getElementById('clearTranscript');
const transcriptInput = document.getElementById('transcriptInput');
const appShell = document.querySelector('.app-shell');
const dictationModelCard = document.getElementById('dictationModelCard');
const dictationModelSelect = document.getElementById('dictationModelSelect');
const installDictationModelBtn = document.getElementById('installDictationModel');
const deleteDictationModelBtn = document.getElementById('deleteDictationModel');
const openWhisperSetupBtn = document.getElementById('openWhisperSetup');
const retryWhisperCheckBtn = document.getElementById('retryWhisperCheck');
const whisperCliHealthEl = document.getElementById('whisperCliHealth');
const dictationModelHealthEl = document.getElementById('dictationModelHealth');
const dictationModelStatusEl = document.getElementById('dictationModelStatus');
const dictationModelBusyEl = document.getElementById('dictationModelBusy');
const dictationDeviceProfileEl = document.getElementById('dictationDeviceProfile');
const dictationModelMetaEl = document.getElementById('dictationModelMeta');

const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition || null;

let recognition = null;
let finalTranscript = '';
let isDictating = false;
let isStartingDictation = false;
let shouldKeepDictating = false;
let restartTimer = null;
let hasMicrophoneAccess = false;
let isInstallingDictationModel = false;
let isDeletingDictationModel = false;
let nativeDictationModelReady = !isFocusedMacDesktopMode();
let whisperCliAvailable = true;
let dictationModels = [];
let currentOnboarding = null;
let setupScreenMode = 'onboarding';
const NATIVE_HOLD_HOTKEYS = new Set(['Fn', 'F19']);
let nativeHotkeyActionInFlight = false;
let nativeHotkeyUnlisten = null;
let nativeFnHoldActive = false;
let nativeFnStopRequested = false;
const MAC_DESKTOP_ONLY_MESSAGE = 'Desktop MVP currently supports macOS only. Current mobile focus is iPhone (iOS).';
const PILL_STATUS_EVENT = 'dicktaint://pill-status';
const FN_HOTKEY_STATE_EVENT = 'dicktaint://fn-state';

function modelDisplayName(model) {
  return String(model?.display_name || '').replace(/\s+\(Selected\)$/u, '').trim();
}

function getTauriInvoke() {
  return window.__TAURI__?.core?.invoke || null;
}

function getTauriEventApi() {
  return window.__TAURI__?.event || null;
}

function isMobileUserAgent() {
  const ua = navigator.userAgent || '';
  return /Android|iPhone|iPad|iPod/i.test(ua);
}

function isNativeDesktopMode() {
  return Boolean(getTauriInvoke()) && !isMobileUserAgent();
}

function isMacDesktopUserAgent() {
  const ua = navigator.userAgent || '';
  return /Macintosh|Mac OS X/i.test(ua);
}

function isFocusedMacDesktopMode() {
  return isNativeDesktopMode() && isMacDesktopUserAgent();
}

function shouldUseTauriCommands() {
  return isFocusedMacDesktopMode();
}

function setUiMode(mode) {
  document.body.dataset.mode = mode;
}

function setStatus(message, tone = 'neutral') {
  statusEl.textContent = message;
  statusEl.dataset.tone = tone;
  syncHotkeyPillForStatus(message, tone);
}

function setHotkeyPill(message, state = 'idle', visible = true) {
  emitHotkeyPillOverlay(message, state, visible);
}

function emitHotkeyPillOverlay(message, state = 'idle', visible = true) {
  const tauriEvent = getTauriEventApi();
  if (typeof tauriEvent?.emit !== 'function') return;

  tauriEvent.emit(PILL_STATUS_EVENT, {
    message: String(message || '').trim() || 'Hold fn to dictate',
    state: String(state || 'idle'),
    visible: Boolean(visible)
  }).catch(() => {});
}

function summarizeHotkeyPillStatus(message, tone = 'neutral') {
  if (!isFocusedMacDesktopMode()) {
    return 'Desktop MVP: macOS only';
  }
  const normalized = String(message || '').toLowerCase();

  if (tone === 'live') {
    return 'Listening - release fn to stop';
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
    return 'Ready - hold fn to start';
  }
  if (tone === 'error') {
    return 'Dictation error - check status';
  }
  return 'Hold fn to dictate';
}

function syncHotkeyPillForStatus(message, tone = 'neutral') {
  if (!isNativeDesktopMode()) {
    setHotkeyPill('', 'idle', false);
    return;
  }
  // Overlay window expects a tight state enum; map richer UI tones into it.
  const state = tone === 'live'
    ? 'live'
    : (tone === 'working' ? 'working' : (tone === 'ok' ? 'ok' : (tone === 'error' ? 'error' : 'idle')));
  setHotkeyPill(summarizeHotkeyPillStatus(message, tone), state, true);
}

function setAppScreen(screen) {
  const next = screen === 'dictation' ? 'dictation' : 'onboarding';
  if (onboardingScreen) onboardingScreen.hidden = next !== 'onboarding';
  if (dictationScreen) dictationScreen.hidden = next !== 'dictation';
  if (appShell) appShell.dataset.screen = next;
  document.body.dataset.screen = next;
}

function setSetupScreenMode(mode) {
  setupScreenMode = mode === 'settings' ? 'settings' : 'onboarding';

  const settingsMode = setupScreenMode === 'settings';
  if (setupModeChipEl) setupModeChipEl.textContent = settingsMode ? 'SETTINGS' : 'ONBOARDING';
  if (setupTitleEl) setupTitleEl.textContent = settingsMode ? 'Manage local speech setup' : 'Set up local speech-to-text';
  if (setupLeadEl) {
    setupLeadEl.textContent = settingsMode
      ? 'Switch models, delete downloads, or re-check whisper-cli. Changes apply to this device only.'
      : 'Everything runs on-device. Pick a model, download it once, and this machine is ready.';
  }
  if (setupStepsEl) setupStepsEl.hidden = settingsMode;
  if (backToDictationBtn) backToDictationBtn.hidden = !settingsMode;
  if (onboardingContinueBtn) onboardingContinueBtn.hidden = settingsMode;
}

function syncFlowForSetupReadiness() {
  const setupReady = !isFocusedMacDesktopMode() || nativeDictationModelReady;
  if (!setupReady) {
    setSetupScreenMode('onboarding');
    setAppScreen('onboarding');
    return;
  }
  if (setupScreenMode === 'onboarding') {
    setAppScreen('dictation');
  }
}

function setDictationModelStatus(message, tone = 'neutral') {
  if (!dictationModelStatusEl) return;
  dictationModelStatusEl.textContent = message;
  dictationModelStatusEl.dataset.tone = tone;
}

function setDictationModelBusy(message = '') {
  if (!dictationModelBusyEl) return;
  const trimmed = String(message || '').trim();
  dictationModelBusyEl.hidden = !trimmed;
  dictationModelBusyEl.textContent = trimmed;
}

function setHealthPill(el, state, message) {
  if (!el) return;
  el.dataset.state = state;
  el.textContent = message;
}

function syncSetupHealthPills() {
  const modelExists = Boolean(currentOnboarding?.selected_model_exists);

  if (!isNativeDesktopMode()) {
    setHealthPill(whisperCliHealthEl, 'ok', 'whisper-cli: n/a (web)');
    setHealthPill(dictationModelHealthEl, 'ok', 'model: n/a (web)');
    return;
  }
  if (!isFocusedMacDesktopMode()) {
    setHealthPill(whisperCliHealthEl, 'error', 'whisper-cli: unsupported on this desktop OS');
    setHealthPill(dictationModelHealthEl, 'error', 'model: unsupported on this desktop OS');
    return;
  }

  if (!currentOnboarding) {
    setHealthPill(whisperCliHealthEl, 'pending', 'whisper-cli: checking');
    setHealthPill(dictationModelHealthEl, 'pending', 'model: checking');
    return;
  }

  if (whisperCliAvailable) {
    setHealthPill(whisperCliHealthEl, 'ok', 'whisper-cli: ready');
  } else {
    setHealthPill(whisperCliHealthEl, 'error', 'whisper-cli: unavailable');
  }

  if (isInstallingDictationModel) {
    setHealthPill(dictationModelHealthEl, 'working', 'model: downloading');
  } else if (isDeletingDictationModel) {
    setHealthPill(dictationModelHealthEl, 'working', 'model: deleting');
  } else if (modelExists) {
    setHealthPill(dictationModelHealthEl, 'ok', 'model: ready');
  } else {
    setHealthPill(dictationModelHealthEl, 'pending', 'model: required');
  }
}

function refreshSelectedModelMeta() {
  if (!dictationModelMetaEl) return;
  const selected = getSelectedDictationModel();
  if (!selected) {
    dictationModelMetaEl.textContent = 'Pick a model to view speed, quality, and local install state.';
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
  dictationModelMetaEl.textContent = parts.join(' • ');
}

function getErrorMessage(error) {
  if (!error) return 'Unknown error';
  const normalize = (value) => {
    if (typeof value !== 'string') return '';
    const trimmed = value.trim();
    if (!trimmed) return '';
    if (trimmed === 'undefined' || trimmed === 'null' || trimmed === '[object Object]') return '';
    return trimmed;
  };

  const direct = normalize(error);
  if (direct) return direct;

  const message = normalize(error.message);
  if (message) return message;

  const nestedError = normalize(error.error);
  if (nestedError) return nestedError;

  try {
    const asJson = JSON.stringify(error);
    if (asJson && asJson !== '{}') return asJson;
  } catch {}
  const fallback = normalize(String(error));
  return fallback || 'Unknown error';
}

function getSelectedDictationModel() {
  const selectedId = (dictationModelSelect?.value || '').trim();
  if (!selectedId) return null;
  return dictationModels.find((model) => model.id === selectedId) || null;
}

function updateModelActionLabels() {
  if (!installDictationModelBtn) return;

  const selected = getSelectedDictationModel();
  if (!selected) {
    installDictationModelBtn.textContent = 'Download + Use';
    return;
  }

  const isCurrent = Boolean(currentOnboarding?.selected_model_exists)
    && currentOnboarding?.selected_model_id === selected.id;

  if (!selected.installed) {
    installDictationModelBtn.textContent = 'Download + Use';
    return;
  }

  installDictationModelBtn.textContent = isCurrent ? 'Using Now' : 'Use Installed';
}

function syncControls() {
  const hasCaptureSupport = isFocusedMacDesktopMode() || (!isNativeDesktopMode() && Boolean(SpeechRecognitionApi));
  const dictationModelMissing = isFocusedMacDesktopMode() && !nativeDictationModelReady;
  const lockControls = isInstallingDictationModel || isDeletingDictationModel;
  const selected = getSelectedDictationModel();
  const setupReady = !isFocusedMacDesktopMode() || nativeDictationModelReady;
  const selectedAlreadyActive = Boolean(selected?.installed)
    && Boolean(currentOnboarding?.selected_model_exists)
    && currentOnboarding?.selected_model_id === selected.id;

  startDictationBtn.disabled = lockControls || !hasCaptureSupport || isDictating || isStartingDictation || dictationModelMissing;
  stopDictationBtn.disabled = lockControls || !hasCaptureSupport || (!isDictating && !isStartingDictation);
  clearTranscriptBtn.disabled = lockControls;

  if (installDictationModelBtn) {
    installDictationModelBtn.disabled = (
      lockControls
      || !dictationModelSelect?.value
      || !whisperCliAvailable
      || selectedAlreadyActive
    );
  }
  if (deleteDictationModelBtn) {
    deleteDictationModelBtn.disabled = lockControls || !selected?.installed;
  }
  if (openWhisperSetupBtn) {
    openWhisperSetupBtn.disabled = lockControls;
  }
  if (retryWhisperCheckBtn) {
    retryWhisperCheckBtn.disabled = lockControls;
  }
  if (dictationModelSelect) {
    dictationModelSelect.disabled = lockControls;
  }
  if (onboardingContinueBtn) {
    onboardingContinueBtn.disabled = lockControls || !setupReady;
    onboardingContinueBtn.textContent = setupReady ? 'Start Dictation' : 'Complete Setup to Continue';
  }
  if (openSettingsBtn) {
    openSettingsBtn.disabled = lockControls || !isFocusedMacDesktopMode();
  }
  if (backToDictationBtn) {
    backToDictationBtn.disabled = lockControls;
  }

  if (appShell) {
    appShell.setAttribute('aria-busy', lockControls ? 'true' : 'false');
  }
  updateModelActionLabels();
  syncSetupHealthPills();
}

function setDictationState(dictating) {
  isDictating = dictating;
  syncControls();
}

function clearRestartTimer() {
  if (!restartTimer) return;
  clearTimeout(restartTimer);
  restartTimer = null;
}

function describeSpeechError(errorCode) {
  if (errorCode === 'not-allowed' || errorCode === 'service-not-allowed') {
    if (hasMicrophoneAccess) {
      return isFocusedMacDesktopMode()
        ? 'Speech recognition permission denied. In macOS Settings > Privacy & Security > Speech Recognition, allow this app/terminal and relaunch.'
        : 'Speech recognition permission denied by browser/runtime. Allow speech recognition and retry.';
    }

    return isFocusedMacDesktopMode()
      ? 'Microphone permission denied. In macOS Settings > Privacy & Security > Microphone, allow this app (or Terminal during tauri:dev), then relaunch.'
      : 'Microphone permission denied in your browser. Allow mic access for this site and try again.';
  }

  if (errorCode === 'audio-capture') {
    return 'No microphone input was found. In macOS Settings > Sound > Input, select your AirPods microphone and retry.';
  }

  if (errorCode === 'no-speech') {
    return 'Mic opened but no speech was detected. Check input level and speak again.';
  }

  if (errorCode === 'network') {
    return 'Speech recognition service was unreachable. Check connectivity or try again in web mode.';
  }

  if (errorCode === 'aborted') {
    return 'Speech recognition stopped unexpectedly.';
  }

  if (errorCode === 'language-not-supported') {
    return 'Speech recognition does not support the configured language.';
  }

  return errorCode || 'unknown';
}

function isFatalSpeechError(errorCode) {
  return ['not-allowed', 'service-not-allowed', 'audio-capture', 'network', 'language-not-supported'].includes(errorCode);
}

async function ensureMicrophoneAccess() {
  if (!navigator.mediaDevices?.getUserMedia) return;

  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  hasMicrophoneAccess = true;
  for (const track of stream.getTracks()) {
    track.stop();
  }
}

function scheduleRecognitionRestart() {
  clearRestartTimer();

  // Browser speech engines can end between utterances; auto-restart keeps hands-free flow.
  restartTimer = setTimeout(() => {
    if (!recognition || !shouldKeepDictating || isStartingDictation || isDictating) return;

    try {
      isStartingDictation = true;
      syncControls();
      recognition.start();
      setUiMode('loading');
      setStatus('Reconnecting dictation...', 'working');
    } catch {
      isStartingDictation = false;
      syncControls();
    }
  }, 250);
}

function describeDeviceProfile(device) {
  if (!device) return '';
  const ram = Number(device.total_memory_gb) || 0;
  const cores = Number(device.logical_cpu_cores) || 1;
  return `${ram} GB RAM • ${cores} logical CPU cores • ${device.architecture || 'unknown arch'} • ${device.os || 'unknown os'}`;
}

function buildDictationModelLabel(model) {
  const fit = model.recommended
    ? 'Recommended'
    : (model.likely_runnable ? 'Likely runnable' : 'Heavy for this machine');
  const local = model.installed ? 'Installed' : `${model.approx_size_gb} GB`;
  return `${modelDisplayName(model)} • ${local} • ${fit}`;
}

function renderDictationModelOptions(models, selectedModelId) {
  if (!dictationModelSelect) return;
  const safeModels = Array.isArray(models) ? models : [];
  dictationModels = safeModels;

  dictationModelSelect.innerHTML = '';
  for (const model of safeModels) {
    const option = document.createElement('option');
    option.value = model.id;
    option.textContent = buildDictationModelLabel(model);
    dictationModelSelect.appendChild(option);
  }

  if (!dictationModelSelect.options.length) {
    dictationModelSelect.value = '';
    updateModelActionLabels();
    refreshSelectedModelMeta();
    return;
  }

  const hasSelectedModel = Boolean(selectedModelId) && safeModels.some((model) => model.id === selectedModelId);
  if (hasSelectedModel) {
    dictationModelSelect.value = selectedModelId;
    updateModelActionLabels();
    refreshSelectedModelMeta();
    return;
  }

  const installed = safeModels.find((model) => model.installed);
  const best = safeModels.find((model) => model.recommended || model.likely_runnable) || safeModels[0];
  dictationModelSelect.value = installed?.id || best?.id || '';
  updateModelActionLabels();
  refreshSelectedModelMeta();
}

async function loadDictationOnboarding({ quietStatus = false } = {}) {
  // Web/mobile bypass desktop onboarding gates and run with browser/manual input paths.
  if (!isNativeDesktopMode()) {
    nativeDictationModelReady = true;
    whisperCliAvailable = true;
    currentOnboarding = null;
    setSetupScreenMode('onboarding');
    if (dictationModelCard) {
      dictationModelCard.hidden = true;
    }
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    setAppScreen('dictation');
    syncControls();
    return null;
  }
  if (!isFocusedMacDesktopMode()) {
    nativeDictationModelReady = false;
    whisperCliAvailable = false;
    currentOnboarding = null;
    setSetupScreenMode('onboarding');
    if (dictationModelCard) {
      dictationModelCard.hidden = true;
    }
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
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
    nativeDictationModelReady = false;
    whisperCliAvailable = false;
    currentOnboarding = null;
    setSetupScreenMode('onboarding');
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    setAppScreen('onboarding');
    syncControls();
    return null;
  }

  try {
    if (!quietStatus) {
      setStatus('Checking local speech-to-text setup...', 'working');
    }
    setDictationModelBusy('');

    const onboarding = await tauriInvoke('get_dictation_onboarding');
    currentOnboarding = onboarding;
    whisperCliAvailable = Boolean(onboarding.whisper_cli_available);
    nativeDictationModelReady = Boolean(onboarding.selected_model_exists && whisperCliAvailable);

    if (dictationModelCard) {
      dictationModelCard.hidden = false;
    }
    if (openSettingsBtn) {
      openSettingsBtn.hidden = false;
    }
    if (dictationDeviceProfileEl) {
      dictationDeviceProfileEl.textContent = describeDeviceProfile(onboarding.device);
    }

    renderDictationModelOptions(onboarding.models, onboarding.selected_model_id);

    if (openWhisperSetupBtn) {
      openWhisperSetupBtn.hidden = Boolean(onboarding.whisper_cli_available);
    }

    if (!onboarding.whisper_cli_available && !onboarding.selected_model_exists) {
      setDictationModelStatus(
        `whisper-cli is unavailable. Packaged builds should include it. In tauri:dev, click "Open CLI Setup (dev)", then "Refresh Setup". Checked: ${onboarding.whisper_cli_path}`,
        'error'
      );
      nativeDictationModelReady = false;
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
    nativeDictationModelReady = false;
    whisperCliAvailable = false;
    currentOnboarding = null;
    setSetupScreenMode('onboarding');
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    setAppScreen('onboarding');
    const details = getErrorMessage(error);
    setDictationModelStatus(`Could not read setup state: ${details}`, 'error');
    setDictationModelBusy('');
    if (!quietStatus) {
      setStatus(`Could not load setup state: ${details}`, 'error');
    }
    syncControls();
    return null;
  }
}

async function installSelectedDictationModel() {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  const selected = getSelectedDictationModel();
  if (!selected) {
    setStatus('Pick a speech model first.', 'error');
    return;
  }
  if (!whisperCliAvailable) {
    setStatus('whisper-cli is unavailable. In tauri:dev, click "Open CLI Setup (dev)", then "Refresh Setup".', 'error');
    return;
  }

  const prevNativeDictationModelReady = nativeDictationModelReady;
  let onboardingAfterInstall = null;

  try {
    isInstallingDictationModel = true;
    syncControls();
    setUiMode('loading');

    const isAlreadyInstalled = Boolean(selected.installed);
    if (isAlreadyInstalled) {
      setDictationModelBusy(`Switching active model to ${modelDisplayName(selected)}...`);
      setDictationModelStatus(`Switching to ${modelDisplayName(selected)}...`, 'neutral');
      setStatus(`Switching active model to ${modelDisplayName(selected)}...`, 'working');
    } else {
      setDictationModelBusy(`Downloading ${modelDisplayName(selected)} (~${selected.approx_size_gb} GB). Keep this window open...`);
      setDictationModelStatus(
        `Downloading ${modelDisplayName(selected)} (~${selected.approx_size_gb} GB). Keep this window open while it downloads...`,
        'neutral'
      );
      setStatus(`Downloading ${modelDisplayName(selected)} model...`, 'working');
    }

    await tauriInvoke('install_dictation_model', { model: selected.id });
    const onboarding = await loadDictationOnboarding({ quietStatus: true });
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
    if (setupScreenMode === 'onboarding') {
      setAppScreen('dictation');
    }
  } catch (error) {
    const details = getErrorMessage(error);
    const modelNoLongerUsable = Boolean(onboardingAfterInstall)
      && (!onboardingAfterInstall.selected_model_exists || !onboardingAfterInstall.whisper_cli_available);
    nativeDictationModelReady = modelNoLongerUsable ? false : prevNativeDictationModelReady;
    setUiMode('error');
    setDictationModelBusy('');
    setDictationModelStatus(`Model update failed: ${details}`, 'error');
    setStatus(`Could not update model: ${details}`, 'error');
  } finally {
    isInstallingDictationModel = false;
    syncControls();
  }
}

async function deleteSelectedDictationModel() {
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
    isDeletingDictationModel = true;
    syncControls();
    setUiMode('loading');
    setDictationModelBusy(`Deleting ${modelDisplayName(selected)} from local storage...`);
    setDictationModelStatus(`Deleting ${modelDisplayName(selected)} from local storage...`, 'neutral');
    setStatus(`Deleting ${modelDisplayName(selected)}...`, 'working');

    await tauriInvoke('delete_dictation_model', { model: selected.id });
    const onboarding = await loadDictationOnboarding({ quietStatus: true });

    if (!onboarding) {
      throw new Error('Delete completed, but setup refresh failed. Click Refresh Setup.');
    }

    if (onboarding.selected_model_exists) {
      const selectedAfter = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
      setStatus(
        `Deleted ${modelDisplayName(selected)}. Active model: ${modelDisplayName(selectedAfter) || onboarding.selected_model_id}.`,
        'ok'
      );
      if (setupScreenMode === 'onboarding') {
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
    isDeletingDictationModel = false;
    syncControls();
  }
}

async function openWhisperSetupPage() {
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

async function startNativeDesktopDictation(trigger = 'button') {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || isDictating || isStartingDictation) return;

  try {
    isStartingDictation = true;
    syncControls();
    setUiMode('loading');
    setStatus('Opening microphone...', 'working');
    await tauriInvoke('start_native_dictation');
    isStartingDictation = false;
    setDictationState(true);
    setUiMode('listening');
    if (trigger === 'hotkey-hold') {
      setStatus('Listening... release fn/F19 to stop and transcribe.', 'live');
    } else if (trigger === 'hotkey') {
      setStatus('Listening... release fn/F19 to stop and transcribe.', 'live');
    } else {
      setStatus('Listening... click Stop to transcribe.', 'live');
    }
  } catch (error) {
    const details = getErrorMessage(error);
    isStartingDictation = false;
    setDictationState(false);
    setUiMode('error');
    setStatus(`Could not start dictation: ${details}`, 'error');
  }
}

async function stopNativeDesktopDictation(trigger = 'button') {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || (!isDictating && !isStartingDictation)) return;

  try {
    setUiMode('loading');
    setStatus('Transcribing captured audio...', 'working');
    const transcript = await tauriInvoke('stop_native_dictation');
    finalTranscript = `${finalTranscript} ${String(transcript || '').trim()}`.trim();
    transcriptInput.value = finalTranscript;
    setUiMode('idle');
    if (trigger === 'hotkey-hold') {
      setStatus('Dictation captured from fn hold and transcribed.', 'ok');
    } else if (trigger === 'hotkey') {
      setStatus('Dictation captured from fn hold and transcribed.', 'ok');
    } else {
      setStatus('Dictation captured and transcribed.', 'ok');
    }
  } catch (error) {
    const details = getErrorMessage(error);
    setUiMode('error');
    setStatus(`Could not stop dictation: ${details}`, 'error');
  } finally {
    isStartingDictation = false;
    setDictationState(false);
  }
}

function isNativeHoldHotkeyKey(value) {
  const key = String(value || '').trim();
  return NATIVE_HOLD_HOTKEYS.has(key);
}

function handleNativeHoldKeydown(event) {
  if (!isFocusedMacDesktopMode()) return;
  if (event.repeat) return;
  if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
  if (!isNativeHoldHotkeyKey(event.key)) return;

  event.preventDefault();
  event.stopPropagation();
  applyNativeFnHoldState(true);
}

function handleNativeHoldKeyup(event) {
  if (!isFocusedMacDesktopMode()) return;
  if (!isNativeHoldHotkeyKey(event.key)) return;

  event.preventDefault();
  event.stopPropagation();
  applyNativeFnHoldState(false);
}

function applyNativeFnHoldState(pressed) {
  if (!isFocusedMacDesktopMode()) return;
  const nextPressed = Boolean(pressed);

  if (nextPressed) {
    nativeFnHoldActive = true;
    nativeFnStopRequested = false;

    if (isDictating || isStartingDictation || nativeHotkeyActionInFlight) return;
    nativeHotkeyActionInFlight = true;
    setHotkeyPill('fn down - starting dictation...', 'working', true);
    Promise.resolve(startNativeDesktopDictation('hotkey-hold'))
      .catch(() => {})
      .finally(() => {
        nativeHotkeyActionInFlight = false;
        if (nativeFnStopRequested || !nativeFnHoldActive) {
          nativeFnStopRequested = false;
          applyNativeFnHoldState(false);
        }
      });
    return;
  }

  nativeFnHoldActive = false;
  if (isStartingDictation && !isDictating) {
    // If release arrives during startup, defer stop until start settles to avoid race drops.
    nativeFnStopRequested = true;
    setHotkeyPill('fn released - waiting for microphone...', 'working', true);
    return;
  }

  if (!isDictating || nativeHotkeyActionInFlight) return;
  nativeHotkeyActionInFlight = true;
  setHotkeyPill('fn released - transcribing...', 'working', true);
  Promise.resolve(stopNativeDesktopDictation('hotkey-hold'))
    .catch(() => {})
    .finally(() => {
      nativeHotkeyActionInFlight = false;
    });
}

function handleNativeFnStateEvent(payload) {
  if (!isFocusedMacDesktopMode()) return;
  applyNativeFnHoldState(Boolean(payload?.pressed));
}

function initDictation() {
  clearTranscriptBtn.addEventListener('click', () => {
    const tauriInvoke = shouldUseTauriCommands() ? getTauriInvoke() : null;
    if (tauriInvoke) {
      tauriInvoke('cancel_native_dictation').catch(() => {});
    }

    shouldKeepDictating = false;
    isStartingDictation = false;
    clearRestartTimer();
    finalTranscript = '';
    transcriptInput.value = '';
    setUiMode('idle');
    setStatus('Transcript cleared.', 'neutral');
  });

  transcriptInput.addEventListener('input', () => {
    finalTranscript = transcriptInput.value.trim();
  });

  if (onboardingContinueBtn) {
    onboardingContinueBtn.addEventListener('click', () => {
      if (isFocusedMacDesktopMode() && !nativeDictationModelReady) {
        setStatus('Complete setup first, then start dictation.', 'neutral');
        return;
      }
      setAppScreen('dictation');
      setStatus('Dictation ready.', 'ok');
    });
  }

  if (openSettingsBtn) {
    openSettingsBtn.addEventListener('click', () => {
      setSetupScreenMode('settings');
      setAppScreen('onboarding');
      setStatus('Settings opened. Manage local model setup here.', 'neutral');
    });
  }

  if (backToDictationBtn) {
    backToDictationBtn.addEventListener('click', () => {
      setAppScreen('dictation');
      setStatus('Back to dictation.', 'neutral');
    });
  }

  if (isFocusedMacDesktopMode()) {
    if (installDictationModelBtn) {
      installDictationModelBtn.addEventListener('click', installSelectedDictationModel);
    }
    if (deleteDictationModelBtn) {
      deleteDictationModelBtn.addEventListener('click', deleteSelectedDictationModel);
    }
    if (openWhisperSetupBtn) {
      openWhisperSetupBtn.addEventListener('click', openWhisperSetupPage);
    }
    if (retryWhisperCheckBtn) {
      retryWhisperCheckBtn.addEventListener('click', () => {
        loadDictationOnboarding();
      });
    }
    if (dictationModelSelect) {
      dictationModelSelect.addEventListener('change', () => {
        const selected = getSelectedDictationModel();
        if (!selected) {
          setDictationModelStatus('Pick a model to manage download/use state.', 'neutral');
        } else if (selected.installed) {
          const isCurrent = Boolean(currentOnboarding?.selected_model_exists)
            && currentOnboarding?.selected_model_id === selected.id;
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

    startDictationBtn.addEventListener('click', () => {
      startNativeDesktopDictation('button');
    });

    stopDictationBtn.addEventListener('click', () => {
      stopNativeDesktopDictation('button');
    });

    window.addEventListener('keydown', handleNativeHoldKeydown, true);
    window.addEventListener('keyup', handleNativeHoldKeyup, true);
    const tauriEvent = getTauriEventApi();
    if (typeof tauriEvent?.listen === 'function' && !nativeHotkeyUnlisten) {
      tauriEvent.listen(FN_HOTKEY_STATE_EVENT, (event) => handleNativeFnStateEvent(event?.payload))
        .then((unlisten) => {
          nativeHotkeyUnlisten = unlisten;
        })
        .catch(() => {});
    }

    syncControls();
    return;
  }

  if (isNativeDesktopMode() && !isFocusedMacDesktopMode()) {
    syncControls();
    return;
  }

  if (!SpeechRecognitionApi) {
    syncControls();
    return;
  }

  recognition = new SpeechRecognitionApi();
  recognition.continuous = true;
  recognition.interimResults = true;
  recognition.lang = 'en-US';

  recognition.onstart = () => {
    isStartingDictation = false;
    setDictationState(true);
    setUiMode('listening');
    setStatus('Listening... speak now.', 'live');
  };

  recognition.onresult = (event) => {
    let interimTranscript = '';

    for (let i = event.resultIndex; i < event.results.length; i += 1) {
      const result = event.results[i];
      const chunk = result[0]?.transcript || '';
      if (result.isFinal) {
        finalTranscript += chunk.trim() ? `${chunk.trim()} ` : '';
      } else {
        interimTranscript += chunk;
      }
    }

    transcriptInput.value = `${finalTranscript}${interimTranscript}`.trim();
  };

  recognition.onerror = (event) => {
    const errorCode = event.error || '';
    const speechError = describeSpeechError(errorCode);
    setDictationState(false);
    isStartingDictation = false;
    syncControls();
    setUiMode('error');
    setStatus(`Dictation error: ${speechError}`, 'error');

    if (isFatalSpeechError(errorCode)) {
      shouldKeepDictating = false;
      clearRestartTimer();
    }
  };

  recognition.onend = () => {
    setDictationState(false);
    isStartingDictation = false;
    syncControls();

    if (shouldKeepDictating) {
      scheduleRecognitionRestart();
      return;
    }

    clearRestartTimer();
    setUiMode('idle');
    setStatus('Dictation stopped.', 'neutral');
  };

  startDictationBtn.addEventListener('click', async () => {
    if (!recognition || isDictating || isStartingDictation) return;

    try {
      shouldKeepDictating = true;
      clearRestartTimer();
      isStartingDictation = true;
      syncControls();
      setUiMode('loading');
      setStatus('Requesting microphone access...', 'working');
      await ensureMicrophoneAccess();
      recognition.start();
      setStatus('Starting dictation...', 'working');
    } catch (error) {
      const details = getErrorMessage(error);
      hasMicrophoneAccess = false;
      shouldKeepDictating = false;
      isStartingDictation = false;
      setDictationState(false);
      setUiMode('error');
      setStatus(`Could not start dictation: ${details}`, 'error');
    }
  });

  stopDictationBtn.addEventListener('click', () => {
    shouldKeepDictating = false;
    isStartingDictation = false;
    clearRestartTimer();
    if (!recognition) return;
    recognition.stop();
    setDictationState(false);
    setUiMode('idle');
    setStatus('Dictation stopped.', 'neutral');
  });
}

async function initApp() {
  await loadDictationOnboarding();
}

setUiMode('loading');
setSetupScreenMode('onboarding');
setAppScreen('onboarding');
syncControls();
syncHotkeyPillForStatus(statusEl.textContent || '', 'neutral');
initDictation();
initApp();
