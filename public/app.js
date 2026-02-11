const statusEl = document.getElementById('status');
const modelSelect = document.getElementById('modelSelect');
const refreshBtn = document.getElementById('refreshModels');
const runBtn = document.getElementById('runRefine');
const startDictationBtn = document.getElementById('startDictation');
const stopDictationBtn = document.getElementById('stopDictation');
const clearTranscriptBtn = document.getElementById('clearTranscript');
const transcriptInput = document.getElementById('transcriptInput');
const output = document.getElementById('output');

const DEFAULT_MODEL = 'karanchopda333/whisper:latest';
const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition || null;

let recognition = null;
let finalTranscript = '';
let isDictating = false;

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

function setStatus(message) {
  statusEl.textContent = message;
}

function setBusy(isBusy) {
  runBtn.disabled = isBusy;
  refreshBtn.disabled = isBusy;
  startDictationBtn.disabled = isBusy || !SpeechRecognitionApi || isDictating;
  stopDictationBtn.disabled = isBusy || !SpeechRecognitionApi || !isDictating;
  clearTranscriptBtn.disabled = isBusy;
}

function setDictationState(dictating) {
  isDictating = dictating;
  startDictationBtn.disabled = !SpeechRecognitionApi || dictating;
  stopDictationBtn.disabled = !SpeechRecognitionApi || !dictating;
}

function pickDefaultModel(models) {
  if (!models.length) return '';
  if (models.includes(DEFAULT_MODEL)) return DEFAULT_MODEL;

  const baseName = DEFAULT_MODEL.split(':')[0];
  const fallbackByBase = models.find((model) => model.startsWith(`${baseName}:`) || model === baseName);
  return fallbackByBase || models[0];
}

function withSpeechSupportHint(message) {
  if (SpeechRecognitionApi) return message;
  return `${message} Speech capture unavailable here; paste or type transcript manually.`;
}

function initDictation() {
  clearTranscriptBtn.addEventListener('click', () => {
    finalTranscript = '';
    transcriptInput.value = '';
    output.value = '';
    setStatus('Transcript cleared.');
  });

  transcriptInput.addEventListener('input', () => {
    finalTranscript = transcriptInput.value.trim();
  });

  if (!SpeechRecognitionApi) {
    startDictationBtn.disabled = true;
    stopDictationBtn.disabled = true;
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
    setStatus(`Dictation error: ${event.error}`);
  };

  recognition.onend = () => {
    if (isDictating) {
      setDictationState(false);
      setStatus('Dictation stopped.');
    }
  };

  startDictationBtn.addEventListener('click', () => {
    try {
      recognition.start();
      setDictationState(true);
      setStatus('Listening... speak now.');
    } catch (error) {
      setStatus(`Could not start dictation: ${error.message}`);
      setDictationState(false);
    }
  });

  stopDictationBtn.addEventListener('click', () => {
    if (!recognition) return;
    recognition.stop();
    setDictationState(false);
    setStatus('Dictation stopped.');
  });
}

async function loadModels() {
  const tauriInvoke = getTauriInvoke();
  const modeLabel = tauriInvoke ? 'desktop mode' : 'web mode';
  setStatus('Loading models from Ollama...');

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
      setStatus(withSpeechSupportHint('Connected. No local models found yet.'));
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
      setStatus(withSpeechSupportHint(`Connected (${modeLabel}). Default model selected: ${DEFAULT_MODEL}`));
    } else {
      setStatus(withSpeechSupportHint(`Connected (${modeLabel}). ${DEFAULT_MODEL} not found, using ${modelSelect.value}.`));
    }
  } catch (error) {
    modelSelect.innerHTML = '';
    const option = document.createElement('option');
    option.value = '';
    option.textContent = 'Unable to connect to Ollama';
    modelSelect.appendChild(option);
    modelSelect.disabled = true;
    setStatus(withSpeechSupportHint(`Connection error: ${error.message}`));
  }
}

async function refineDictation() {
  const tauriInvoke = getTauriInvoke();
  const model = modelSelect.value;
  const transcript = transcriptInput.value.trim();
  const instruction = 'Clean this raw dictation transcript into readable text with punctuation. Keep intent and wording natural.';

  if (!model) {
    setStatus('Pick a model first.');
    return;
  }

  if (!transcript) {
    setStatus('Paste a transcript before running.');
    return;
  }

  setBusy(true);
  setStatus('Cleaning transcript...');
  output.value = '';

  try {
    const text = tauriInvoke
      ? await tauriInvoke('refine_dictation', { model, transcript, instruction })
      : await refineViaHttp(model, transcript, instruction);

    output.value = text;
    setStatus('Done. Clean dictation output generated.');
  } catch (error) {
    setStatus(`Run failed: ${error.message}`);
  } finally {
    setBusy(false);
  }
}

refreshBtn.addEventListener('click', loadModels);
runBtn.addEventListener('click', refineDictation);

initDictation();
loadModels();
