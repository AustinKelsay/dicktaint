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

const {
  DEFAULT_MODEL = 'karanchopda333/whisper:latest',
  pickDefaultModel = (models) => (Array.isArray(models) && models[0]) || '',
  withSpeechSupportHint = (message) => message
} = window.DictationLogic || {};
const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition || null;

let recognition = null;
let finalTranscript = '';
let isDictating = false;
let isBusy = false;
const runBtnLabel = runBtn.textContent;

function getTauriInvoke() {
  return window.__TAURI__?.core?.invoke || null;
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

function syncControls() {
  runBtn.disabled = isBusy;
  refreshBtn.disabled = isBusy;
  startDictationBtn.disabled = isBusy || !SpeechRecognitionApi || isDictating;
  stopDictationBtn.disabled = isBusy || !SpeechRecognitionApi || !isDictating;
  clearTranscriptBtn.disabled = isBusy;

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


function initDictation() {
  clearTranscriptBtn.addEventListener('click', () => {
    finalTranscript = '';
    transcriptInput.value = '';
    output.value = '';
    setUiMode('idle');
    setStatus('Transcript cleared.', 'neutral');
  });

  transcriptInput.addEventListener('input', () => {
    finalTranscript = transcriptInput.value.trim();
  });

  if (!SpeechRecognitionApi) {
    syncControls();
    return;
  }

  recognition = new SpeechRecognitionApi();
  recognition.continuous = true;
  recognition.interimResults = true;
  recognition.lang = 'en-US';

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
    setDictationState(false);
    setUiMode('error');
    setStatus(`Dictation error: ${event.error}`, 'error');
  };

  recognition.onend = () => {
    if (isDictating) {
      setDictationState(false);
      setUiMode('idle');
      setStatus('Dictation stopped.', 'neutral');
    }
  };

  startDictationBtn.addEventListener('click', () => {
    try {
      recognition.start();
      setDictationState(true);
      setUiMode('listening');
      setStatus('Listening... speak now.', 'live');
    } catch (error) {
      setDictationState(false);
      setUiMode('error');
      setStatus(`Could not start dictation: ${error.message}`, 'error');
    }
  });

  stopDictationBtn.addEventListener('click', () => {
    if (!recognition) return;
    recognition.stop();
    setDictationState(false);
    setUiMode('idle');
    setStatus('Dictation stopped.', 'neutral');
  });
}

async function loadModels() {
  const tauriInvoke = getTauriInvoke();
  const modeLabel = tauriInvoke ? 'desktop mode' : 'web mode';
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
      setStatus(withSpeechSupportHint('Connected. No local models found yet.', Boolean(SpeechRecognitionApi)), 'ok');
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
      setStatus(withSpeechSupportHint(`Connected (${modeLabel}). Default model selected: ${DEFAULT_MODEL}`, Boolean(SpeechRecognitionApi)), 'ok');
    } else {
      setUiMode('idle');
      setStatus(withSpeechSupportHint(`Connected (${modeLabel}). ${DEFAULT_MODEL} not found, using ${modelSelect.value}.`, Boolean(SpeechRecognitionApi)), 'ok');
    }
  } catch (error) {
    modelSelect.innerHTML = '';
    const option = document.createElement('option');
    option.value = '';
    option.textContent = 'Unable to connect to Ollama';
    modelSelect.appendChild(option);
    modelSelect.disabled = true;
    setUiMode('error');
    setStatus(withSpeechSupportHint(`Connection error: ${error.message}`, Boolean(SpeechRecognitionApi)), 'error');
  }
}

async function refineDictation() {
  const tauriInvoke = getTauriInvoke();
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

setUiMode('loading');
syncControls();
initDictation();
loadModels();
