const statusEl = document.getElementById('status');
const modelSelect = document.getElementById('modelSelect');
const refreshBtn = document.getElementById('refreshModels');
const runBtn = document.getElementById('runRefine');
const startDictationBtn = document.getElementById('startDictation');
const stopDictationBtn = document.getElementById('stopDictation');
const clearTranscriptBtn = document.getElementById('clearTranscript');
const transcriptInput = document.getElementById('transcriptInput');
const output = document.getElementById('output');
const appShell = document.querySelector('.app-shell');
const dictationModelCard = document.getElementById('dictationModelCard');
const dictationModelSelect = document.getElementById('dictationModelSelect');
const installDictationModelBtn = document.getElementById('installDictationModel');
const dictationModelStatusEl = document.getElementById('dictationModelStatus');
const dictationDeviceProfileEl = document.getElementById('dictationDeviceProfile');

const {
  DEFAULT_MODEL = 'llama3.2:3b',
  pickDefaultModel = (models) => (Array.isArray(models) && models[0]) || '',
  withSpeechSupportHint = (message) => message
} = window.DictationLogic || {};
const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition || null;

let recognition = null;
let finalTranscript = '';
let isDictating = false;
let isStartingDictation = false;
let isBusy = false;
let shouldKeepDictating = false;
let restartTimer = null;
let hasMicrophoneAccess = false;
let isInstallingDictationModel = false;
let nativeDictationModelReady = !isNativeDesktopMode();
const runBtnLabel = runBtn.textContent;

function getTauriInvoke() {
  return window.__TAURI__?.core?.invoke || null;
}

function isMobileUserAgent() {
  const ua = navigator.userAgent || '';
  return /Android|iPhone|iPad|iPod/i.test(ua);
}

function isNativeDesktopMode() {
  return Boolean(getTauriInvoke()) && !isMobileUserAgent();
}

function shouldUseTauriCommands() {
  return isNativeDesktopMode();
}

async function listModelsViaHttp() {
  const response = await fetch('/api/models');
  const data = await response.json();

  if (!response.ok || !data.ok) {
    throw new Error(data.error || 'Failed to load models');
  }

  return data.models || [];
}

async function refineViaHttp(model, transcript, instruction) {
  const response = await fetch('/api/refine', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      model,
      transcript,
      instruction
    })
  });

  const data = await response.json();

  if (!response.ok || !data.ok) {
    throw new Error(data.error || 'Refine request failed');
  }

  return data.text || '';
}

function setUiMode(mode) {
  document.body.dataset.mode = mode;
}

function setStatus(message, tone = 'neutral') {
  statusEl.textContent = message;
  statusEl.dataset.tone = tone;
}

function setDictationModelStatus(message, tone = 'neutral') {
  if (!dictationModelStatusEl) return;
  dictationModelStatusEl.textContent = message;
  dictationModelStatusEl.dataset.tone = tone;
}

function syncControls() {
  const hasCaptureSupport = isNativeDesktopMode() || Boolean(SpeechRecognitionApi);
  const dictationModelMissing = isNativeDesktopMode() && !nativeDictationModelReady;
  const lockControls = isBusy || isInstallingDictationModel;
  runBtn.disabled = lockControls;
  refreshBtn.disabled = lockControls;
  startDictationBtn.disabled = lockControls || !hasCaptureSupport || isDictating || isStartingDictation || dictationModelMissing;
  stopDictationBtn.disabled = lockControls || !hasCaptureSupport || (!isDictating && !isStartingDictation);
  clearTranscriptBtn.disabled = lockControls;
  if (installDictationModelBtn) {
    installDictationModelBtn.disabled = lockControls || !dictationModelSelect?.value;
  }
  if (dictationModelSelect) {
    dictationModelSelect.disabled = lockControls;
  }

  runBtn.dataset.busy = isBusy ? 'true' : 'false';
  runBtn.textContent = isBusy ? 'Polishing...' : runBtnLabel;
  if (appShell) {
    appShell.setAttribute('aria-busy', isBusy ? 'true' : 'false');
  }
}

function setBusy(busy) {
  isBusy = Boolean(busy);
  syncControls();
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
      return isNativeDesktopMode()
        ? 'Speech recognition permission denied. In macOS Settings > Privacy & Security > Speech Recognition, allow this app/terminal and relaunch.'
        : 'Speech recognition permission denied by browser/runtime. Allow speech recognition and retry.';
    }

    return isNativeDesktopMode()
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
  const performance = model.recommended
    ? 'Recommended for this device'
    : (model.likely_runnable ? `Runs on >= ${model.min_ram_gb} GB RAM` : `Likely too heavy (< ${model.min_ram_gb} GB RAM)`);
  const installed = model.installed ? 'Installed' : 'Not installed';
  return `${model.display_name} • ${model.approx_size_gb} GB • ${performance} • ${installed}`;
}

function renderDictationModelOptions(models, selectedModelId) {
  if (!dictationModelSelect) return;

  dictationModelSelect.innerHTML = '';
  for (const model of models || []) {
    const option = document.createElement('option');
    option.value = model.id;
    option.textContent = buildDictationModelLabel(model);
    dictationModelSelect.appendChild(option);
  }

  if (selectedModelId) {
    dictationModelSelect.value = selectedModelId;
  } else if (dictationModelSelect.options.length > 0) {
    const best = (models || []).find((model) => model.recommended || model.likely_runnable) || models[0];
    dictationModelSelect.value = best?.id || '';
  }
}

async function loadDictationOnboarding({ quietStatus = false } = {}) {
  if (!isNativeDesktopMode()) {
    nativeDictationModelReady = true;
    if (dictationModelCard) {
      dictationModelCard.hidden = true;
    }
    syncControls();
    return null;
  }

  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke) {
    nativeDictationModelReady = false;
    syncControls();
    return null;
  }

  try {
    if (!quietStatus) {
      setStatus('Checking local dictation model setup...', 'working');
    }
    const onboarding = await tauriInvoke('get_dictation_onboarding');
    nativeDictationModelReady = Boolean(onboarding.selected_model_exists);

    if (dictationModelCard) {
      dictationModelCard.hidden = false;
    }
    if (dictationDeviceProfileEl) {
      dictationDeviceProfileEl.textContent = describeDeviceProfile(onboarding.device);
    }
    renderDictationModelOptions(onboarding.models, onboarding.selected_model_id);

    if (!onboarding.wispr_cli_available && !onboarding.selected_model_exists) {
      setDictationModelStatus(
        `Wispr CLI not found at "${onboarding.wispr_cli_path}". Install it or set WISPR_CLI_PATH.`,
        'error'
      );
      nativeDictationModelReady = false;
    } else if (!onboarding.wispr_cli_available && onboarding.selected_model_exists) {
      setDictationModelStatus(
        `Selected model is ready. Wispr CLI missing at "${onboarding.wispr_cli_path}", so you cannot pull a different model until it is installed.`,
        'neutral'
      );
    } else if (onboarding.selected_model_exists) {
      const selected = (onboarding.models || []).find((item) => item.id === onboarding.selected_model_id);
      setDictationModelStatus(
        `Ready on this device: ${selected?.display_name || onboarding.selected_model_id}.`,
        'ok'
      );
      if (!quietStatus) {
        setStatus('Dictation model ready. You can start dictation.', 'ok');
      }
    } else {
      setDictationModelStatus(
        'Select a model and download it to this device before starting dictation.',
        'neutral'
      );
      if (!quietStatus) {
        setStatus('Onboarding required: choose and download a local dictation model.', 'neutral');
      }
    }

    syncControls();
    return onboarding;
  } catch (error) {
    nativeDictationModelReady = false;
    setDictationModelStatus(`Could not read onboarding state: ${error.message}`, 'error');
    if (!quietStatus) {
      setStatus(`Could not load dictation onboarding: ${error.message}`, 'error');
    }
    syncControls();
    return null;
  }
}

async function installSelectedDictationModel() {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isNativeDesktopMode()) return;
  const model = (dictationModelSelect?.value || '').trim();
  if (!model) {
    setStatus('Pick a dictation model first.', 'error');
    return;
  }

  try {
    isInstallingDictationModel = true;
    syncControls();
    setUiMode('loading');
    setDictationModelStatus('Downloading model to local device storage...', 'neutral');
    setStatus('Downloading dictation model with Wispr CLI...', 'working');
    await tauriInvoke('install_dictation_model', { model });
    await loadDictationOnboarding({ quietStatus: true });
    setUiMode('idle');
    setStatus('Dictation model downloaded and selected for this device.', 'ok');
  } catch (error) {
    nativeDictationModelReady = false;
    setUiMode('error');
    setDictationModelStatus(`Install failed: ${error.message}`, 'error');
    setStatus(`Could not install dictation model: ${error.message}`, 'error');
  } finally {
    isInstallingDictationModel = false;
    syncControls();
  }
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
    output.value = '';
    setUiMode('idle');
    setStatus('Transcript cleared.', 'neutral');
  });

  transcriptInput.addEventListener('input', () => {
    finalTranscript = transcriptInput.value.trim();
  });

  if (isNativeDesktopMode()) {
    if (installDictationModelBtn) {
      installDictationModelBtn.addEventListener('click', installSelectedDictationModel);
    }
    if (dictationModelSelect) {
      dictationModelSelect.addEventListener('change', () => {
        const selectedLabel = dictationModelSelect.selectedOptions[0]?.textContent || 'Selected model';
        setDictationModelStatus(`${selectedLabel}`, 'neutral');
        syncControls();
      });
    }

    startDictationBtn.addEventListener('click', async () => {
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
        setStatus('Listening... click Stop to transcribe.', 'live');
      } catch (error) {
        isStartingDictation = false;
        setDictationState(false);
        setUiMode('error');
        setStatus(`Could not start dictation: ${error.message}`, 'error');
      }
    });

    stopDictationBtn.addEventListener('click', async () => {
      const tauriInvoke = getTauriInvoke();
      if (!tauriInvoke || (!isDictating && !isStartingDictation)) return;

      try {
        setUiMode('loading');
        setStatus('Transcribing captured audio...', 'working');
        const transcript = await tauriInvoke('stop_native_dictation');
        finalTranscript = `${finalTranscript} ${String(transcript || '').trim()}`.trim();
        transcriptInput.value = finalTranscript;
        setUiMode('idle');
        setStatus('Dictation captured and transcribed.', 'ok');
      } catch (error) {
        setUiMode('error');
        setStatus(`Could not stop dictation: ${error.message}`, 'error');
      } finally {
        isStartingDictation = false;
        setDictationState(false);
      }
    });

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
      hasMicrophoneAccess = false;
      shouldKeepDictating = false;
      isStartingDictation = false;
      setDictationState(false);
      setUiMode('error');
      setStatus(`Could not start dictation: ${error.message}`, 'error');
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

async function loadModels() {
  const useTauri = shouldUseTauriCommands();
  const tauriInvoke = useTauri ? getTauriInvoke() : null;
  const hasLiveCapture = useTauri || Boolean(SpeechRecognitionApi);
  const modeLabel = useTauri
    ? 'desktop mode'
    : (getTauriInvoke() ? 'mobile mode' : 'web mode');
  setUiMode('loading');
  setStatus('Loading models from Ollama...', 'working');

  try {
    const models = tauriInvoke
      ? await tauriInvoke('list_models')
      : await listModelsViaHttp();

    modelSelect.innerHTML = '';

    if (!models.length) {
      const option = document.createElement('option');
      option.value = '';
      option.textContent = 'No models found (run ollama pull <model>)';
      modelSelect.appendChild(option);
      modelSelect.disabled = true;
      setUiMode('idle');
      setStatus(withSpeechSupportHint('Connected. No local models found yet.', hasLiveCapture), 'ok');
      return;
    }

    for (const modelName of models) {
      const option = document.createElement('option');
      option.value = modelName;
      option.textContent = modelName;
      modelSelect.appendChild(option);
    }

    modelSelect.value = pickDefaultModel(models);
    modelSelect.disabled = false;
    if (modelSelect.value === DEFAULT_MODEL) {
      setUiMode('idle');
      setStatus(withSpeechSupportHint(`Connected (${modeLabel}). Default model selected: ${DEFAULT_MODEL}`, hasLiveCapture), 'ok');
    } else {
      setUiMode('idle');
      setStatus(withSpeechSupportHint(`Connected (${modeLabel}). ${DEFAULT_MODEL} not found, using ${modelSelect.value}.`, hasLiveCapture), 'ok');
    }
  } catch (error) {
    modelSelect.innerHTML = '';
    const option = document.createElement('option');
    option.value = '';
    option.textContent = 'Unable to connect to Ollama';
    modelSelect.appendChild(option);
    modelSelect.disabled = true;
    setUiMode('error');
    setStatus(withSpeechSupportHint(`Connection error: ${error.message}`, hasLiveCapture), 'error');
  }
}

async function refineDictation() {
  const useTauri = shouldUseTauriCommands();
  const tauriInvoke = useTauri ? getTauriInvoke() : null;
  const model = modelSelect.value;
  const transcript = transcriptInput.value.trim();
  const instruction = 'Clean this raw dictation transcript into readable text with punctuation. Keep intent and wording natural.';

  if (!model) {
    setUiMode('error');
    setStatus('Pick a model first.', 'error');
    return;
  }

  if (!transcript) {
    setUiMode('error');
    setStatus('Paste a transcript before running.', 'error');
    return;
  }

  setBusy(true);
  setUiMode('refining');
  setStatus('Cleaning transcript...', 'working');
  output.value = '';

  try {
    const text = tauriInvoke
      ? await tauriInvoke('refine_dictation', { model, transcript, instruction })
      : await refineViaHttp(model, transcript, instruction);

    output.value = text;
    setUiMode('success');
    setStatus('Done. Clean dictation output generated.', 'ok');
  } catch (error) {
    setUiMode('error');
    setStatus(`Run failed: ${error.message}`, 'error');
  } finally {
    setBusy(false);
  }
}

refreshBtn.addEventListener('click', loadModels);
runBtn.addEventListener('click', refineDictation);

async function initApp() {
  await loadModels();
  await loadDictationOnboarding();
}

setUiMode('loading');
syncControls();
initDictation();
initApp();
