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
const dictationHotkeyCardEl = document.getElementById('dictationHotkeyCard');
const dictationHotkeyInputEl = document.getElementById('dictationHotkeyInput');
const recordDictationHotkeyBtn = document.getElementById('recordDictationHotkey');
const saveDictationHotkeyBtn = document.getElementById('saveDictationHotkey');
const resetDictationHotkeyBtn = document.getElementById('resetDictationHotkey');
const clearDictationHotkeyBtn = document.getElementById('clearDictationHotkey');
const dictationHotkeyStatusEl = document.getElementById('dictationHotkeyStatus');
const quickDictationFab = document.getElementById('quickDictationFab');

const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition || null;
const DEFAULT_DICTATION_HOTKEY = isMacPlatform() ? 'Fn' : 'CmdOrCtrl+Shift+D';
const HOTKEY_MODIFIER_ORDER = ['CmdOrCtrl', 'Cmd', 'Ctrl', 'Alt', 'Shift', 'Super'];
const DICTATION_HOTKEY_EVENT = 'dictation:hotkey-triggered';
const DICTATION_STATE_EVENT = 'dictation:state-changed';

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
let savedDictationHotkey = null;
let defaultDictationHotkey = DEFAULT_DICTATION_HOTKEY;
let activeHotkeySpec = null;
let pendingDictationHotkey = '';
let isCapturingDictationHotkey = false;
let lastHotkeyToggleAtMs = 0;

function modelDisplayName(model) {
  return String(model?.display_name || '').replace(/\s+\(Selected\)$/u, '').trim();
}

function isMacPlatform() {
  return /Mac|iPhone|iPad|iPod/i.test(navigator.platform || navigator.userAgent || '');
}

function setDictationHotkeyStatus(message, tone = 'neutral') {
  if (!dictationHotkeyStatusEl) return;
  dictationHotkeyStatusEl.textContent = message;
  dictationHotkeyStatusEl.dataset.tone = tone;
}

function normalizeHotkeyModifier(token) {
  const normalized = String(token || '').trim().toLowerCase();
  if (!normalized) return null;

  if (['cmdorctrl', 'commandorcontrol', 'primary', 'mod'].includes(normalized)) return 'CmdOrCtrl';
  if (['cmd', 'command'].includes(normalized)) return 'Cmd';
  if (['ctrl', 'control'].includes(normalized)) return 'Ctrl';
  if (['alt', 'option'].includes(normalized)) return 'Alt';
  if (normalized === 'shift') return 'Shift';
  if (['super', 'meta', 'win', 'windows'].includes(normalized)) return 'Super';
  return null;
}

function normalizeHotkeyKey(token) {
  const trimmed = String(token || '').trim();
  if (!trimmed) return null;

  if (/^[a-z0-9]$/i.test(trimmed)) return trimmed.toUpperCase();

  const lower = trimmed.toLowerCase();
  const aliasMap = {
    fn: 'Fn',
    function: 'Fn',
    globe: 'Fn',
    space: 'Space',
    tab: 'Tab',
    enter: 'Enter',
    return: 'Enter',
    esc: 'Escape',
    escape: 'Escape',
    backspace: 'Backspace',
    delete: 'Delete',
    del: 'Delete',
    up: 'Up',
    arrowup: 'Up',
    down: 'Down',
    arrowdown: 'Down',
    left: 'Left',
    arrowleft: 'Left',
    right: 'Right',
    arrowright: 'Right',
    home: 'Home',
    end: 'End',
    pageup: 'PageUp',
    pagedown: 'PageDown',
    insert: 'Insert'
  };
  if (aliasMap[lower]) return aliasMap[lower];

  const functionKeyMatch = /^f([1-9]|1\d|2[0-4])$/i.exec(trimmed);
  if (functionKeyMatch) {
    return `F${functionKeyMatch[1]}`;
  }

  return null;
}

function parseHotkeyCombo(raw) {
  const source = String(raw || '').trim();
  if (!source) {
    return { ok: false, error: 'Hotkey cannot be empty.' };
  }

  const modifiers = new Set();
  let key = null;

  for (const token of source.split('+')) {
    const trimmed = token.trim();
    if (!trimmed) {
      return { ok: false, error: 'Hotkey contains an empty token.' };
    }

    const modifier = normalizeHotkeyModifier(trimmed);
    if (modifier) {
      if (key) return { ok: false, error: 'Modifiers must come before the main key.' };
      modifiers.add(modifier);
      continue;
    }

    if (key) {
      return { ok: false, error: 'Hotkey can only have one main key.' };
    }
    key = normalizeHotkeyKey(trimmed);
    if (!key) {
      return { ok: false, error: `Unsupported key "${trimmed}".` };
    }
  }

  if (key === 'Fn') {
    if (modifiers.size) return { ok: false, error: 'Fn hotkey must be used by itself.' };
    return {
      ok: true,
      display: 'Fn',
      key: 'Fn',
      requires: {
        cmdOrCtrl: false,
        cmd: false,
        ctrl: false,
        alt: false,
        shift: false,
        super: false
      }
    };
  }

  if (!modifiers.size) return { ok: false, error: 'Hotkey must include at least one modifier (or use Fn by itself on macOS).' };
  if (!key) return { ok: false, error: 'Hotkey is missing its main key.' };
  if (modifiers.has('CmdOrCtrl') && (modifiers.has('Cmd') || modifiers.has('Ctrl'))) {
    return { ok: false, error: 'Use CmdOrCtrl by itself, or use Cmd/Ctrl explicitly.' };
  }

  const orderedModifiers = HOTKEY_MODIFIER_ORDER.filter((modifier) => modifiers.has(modifier));
  const display = [...orderedModifiers, key].join('+');
  return {
    ok: true,
    display,
    key,
    requires: {
      cmdOrCtrl: modifiers.has('CmdOrCtrl'),
      cmd: modifiers.has('Cmd'),
      ctrl: modifiers.has('Ctrl'),
      alt: modifiers.has('Alt'),
      shift: modifiers.has('Shift'),
      super: modifiers.has('Super')
    }
  };
}

function eventKeyToken(event) {
  return normalizeHotkeyKey(event.key);
}

function buildHotkeyFromEvent(event) {
  const key = eventKeyToken(event);
  if (!key) return null;
  if (key === 'Fn') return 'Fn';

  const keyName = String(event.key || '').toLowerCase();
  if (['shift', 'control', 'meta', 'alt', 'super'].includes(keyName)) return null;

  const isMac = isMacPlatform();
  const modifiers = [];

  const primaryPressed = isMac ? event.metaKey : event.ctrlKey;
  if (primaryPressed) modifiers.push('CmdOrCtrl');
  if (event.altKey) modifiers.push('Alt');
  if (event.shiftKey) modifiers.push('Shift');

  if (!modifiers.length) return null;
  return [...modifiers, key].join('+');
}

function eventMatchesHotkey(event, spec) {
  if (!spec?.ok) return false;
  if (event.repeat) return false;
  if (spec.key === 'Fn') {
    const key = eventKeyToken(event);
    const keyName = String(event.key || '').toLowerCase();
    const fnPressed = key === 'Fn'
      || keyName === 'fn'
      || (keyName === 'unidentified' && Boolean(event.getModifierState?.('Fn')));
    if (!fnPressed) return false;
    return !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
  }

  const key = eventKeyToken(event);
  if (!key || key !== spec.key) return false;

  const isMac = isMacPlatform();
  const expectedCtrl = Boolean(spec.requires.ctrl || (spec.requires.cmdOrCtrl && !isMac));
  const expectedMeta = Boolean(
    spec.requires.cmd
      || spec.requires.super
      || (spec.requires.cmdOrCtrl && isMac)
  );

  if (event.ctrlKey !== expectedCtrl) return false;
  if (event.metaKey !== expectedMeta) return false;
  if (event.altKey !== Boolean(spec.requires.alt)) return false;
  if (event.shiftKey !== Boolean(spec.requires.shift)) return false;
  return true;
}

function syncDictationHotkeyUi() {
  const nativeDesktop = isNativeDesktopMode();
  if (dictationHotkeyCardEl) dictationHotkeyCardEl.hidden = !nativeDesktop;
  if (!nativeDesktop) return;

  if (dictationHotkeyInputEl) {
    dictationHotkeyInputEl.value = pendingDictationHotkey || savedDictationHotkey || '';
    dictationHotkeyInputEl.placeholder = defaultDictationHotkey || DEFAULT_DICTATION_HOTKEY;
  }
  if (recordDictationHotkeyBtn) {
    recordDictationHotkeyBtn.textContent = isCapturingDictationHotkey ? 'Press Keys...' : 'Record';
  }
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
  const hotkeyDisabled = lockControls || !isNativeDesktopMode();
  const selected = getSelectedDictationModel();
  const setupReady = !isFocusedMacDesktopMode() || nativeDictationModelReady;
  const selectedAlreadyActive = Boolean(selected?.installed)
    && Boolean(currentOnboarding?.selected_model_exists)
    && currentOnboarding?.selected_model_id === selected.id;
  const normalizedPending = String(pendingDictationHotkey || '').trim();
  const normalizedSaved = String(savedDictationHotkey || '').trim();
  const pendingMatchesSaved = normalizedPending === normalizedSaved;
  const hasPendingHotkey = Boolean(normalizedPending);
  const savedIsDefault = Boolean(normalizedSaved && defaultDictationHotkey && normalizedSaved === defaultDictationHotkey);

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
  if (recordDictationHotkeyBtn) {
    recordDictationHotkeyBtn.disabled = hotkeyDisabled;
  }
  if (saveDictationHotkeyBtn) {
    saveDictationHotkeyBtn.disabled = hotkeyDisabled || !hasPendingHotkey || pendingMatchesSaved || isCapturingDictationHotkey;
  }
  if (resetDictationHotkeyBtn) {
    resetDictationHotkeyBtn.disabled = hotkeyDisabled || savedIsDefault;
  }
  if (clearDictationHotkeyBtn) {
    clearDictationHotkeyBtn.disabled = hotkeyDisabled || !normalizedSaved;
  }

  if (appShell) {
    appShell.setAttribute('aria-busy', lockControls ? 'true' : 'false');
  }
  if (quickDictationFab) {
    if (isNativeDesktopMode()) {
      quickDictationFab.hidden = true; // pill window handles this in native desktop mode
    } else {
      quickDictationFab.disabled = startDictationBtn.disabled && stopDictationBtn.disabled;
      quickDictationFab.setAttribute('aria-label',
        isDictating || isStartingDictation ? 'Stop dictation' : 'Start dictation');
    }
  }
  syncDictationHotkeyUi();
  updateModelActionLabels();
  syncSetupHealthPills();
}

function setDictationState(dictating) {
  isDictating = dictating;
  syncControls();
}

function appendTranscriptChunk(chunk) {
  const trimmed = String(chunk || '').trim();
  if (!trimmed) return false;
  finalTranscript = `${finalTranscript} ${trimmed}`.trim();
  transcriptInput.value = finalTranscript;
  return true;
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

function applyDictationHotkeyPayload(payload, { preservePending = false } = {}) {
  const rawDefault = String(
    payload?.default_trigger
      || payload?.default_dictation_trigger
      || DEFAULT_DICTATION_HOTKEY
  ).trim();
  const parsedDefault = parseHotkeyCombo(rawDefault);
  defaultDictationHotkey = parsedDefault.ok ? parsedDefault.display : DEFAULT_DICTATION_HOTKEY;

  const rawTrigger = String(payload?.trigger || payload?.dictation_trigger || '').trim();
  if (!rawTrigger) {
    savedDictationHotkey = null;
    activeHotkeySpec = null;
    if (!preservePending) pendingDictationHotkey = '';
    setDictationHotkeyStatus(`Hotkey disabled. Default: ${defaultDictationHotkey}.`, 'neutral');
    syncControls();
    return;
  }

  const parsedTrigger = parseHotkeyCombo(rawTrigger);
  if (!parsedTrigger.ok) {
    savedDictationHotkey = null;
    activeHotkeySpec = null;
    if (!preservePending) pendingDictationHotkey = '';
    setDictationHotkeyStatus(`Saved hotkey ignored: ${parsedTrigger.error}`, 'error');
    syncControls();
    return;
  }

  savedDictationHotkey = parsedTrigger.display;
  activeHotkeySpec = parsedTrigger;
  if (!preservePending) pendingDictationHotkey = parsedTrigger.display;
  setDictationHotkeyStatus(`Current hotkey: ${parsedTrigger.display}`, 'ok');
  syncControls();
}

function beginDictationHotkeyCapture() {
  if (!isNativeDesktopMode()) return;
  isCapturingDictationHotkey = true;
  setDictationHotkeyStatus('Press your desired key combo now. Press Escape to cancel.', 'neutral');
  syncControls();
}

function cancelDictationHotkeyCapture() {
  if (!isCapturingDictationHotkey) return;
  isCapturingDictationHotkey = false;
  if (savedDictationHotkey) {
    setDictationHotkeyStatus(`Current hotkey: ${savedDictationHotkey}`, 'ok');
  } else {
    setDictationHotkeyStatus(`Hotkey disabled. Default: ${defaultDictationHotkey}.`, 'neutral');
  }
  syncControls();
}

async function saveDictationHotkey(value) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isNativeDesktopMode()) return;

  const parsed = parseHotkeyCombo(value);
  if (!parsed.ok) {
    setDictationHotkeyStatus(parsed.error, 'error');
    setStatus(`Could not save hotkey: ${parsed.error}`, 'error');
    return;
  }

  try {
    const payload = await tauriInvoke('set_dictation_trigger', { trigger: parsed.display });
    applyDictationHotkeyPayload(payload);
    setStatus(`Dictation hotkey saved: ${parsed.display}`, 'ok');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not save hotkey: ${details}`, 'error');
    setStatus(`Could not save hotkey: ${details}`, 'error');
  } finally {
    isCapturingDictationHotkey = false;
    syncControls();
  }
}

async function clearDictationHotkey() {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isNativeDesktopMode()) return;

  try {
    const payload = await tauriInvoke('clear_dictation_trigger');
    applyDictationHotkeyPayload(payload);
    setStatus('Dictation hotkey disabled.', 'neutral');
  } catch (error) {
    const details = getErrorMessage(error);
    setDictationHotkeyStatus(`Could not disable hotkey: ${details}`, 'error');
    setStatus(`Could not disable hotkey: ${details}`, 'error');
  } finally {
    isCapturingDictationHotkey = false;
    syncControls();
  }
}

async function loadDictationOnboarding({ quietStatus = false } = {}) {
  // Web/mobile bypass desktop onboarding gates and run with browser/manual input paths.
  if (!isNativeDesktopMode()) {
    nativeDictationModelReady = true;
    whisperCliAvailable = true;
    currentOnboarding = null;
    savedDictationHotkey = null;
    pendingDictationHotkey = '';
    activeHotkeySpec = null;
    isCapturingDictationHotkey = false;
    setSetupScreenMode('onboarding');
    if (dictationModelCard) {
      dictationModelCard.hidden = true;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = true;
    }
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    setAppScreen('dictation');
    syncDictationHotkeyUi();
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
    savedDictationHotkey = null;
    pendingDictationHotkey = '';
    activeHotkeySpec = null;
    isCapturingDictationHotkey = false;
    setSetupScreenMode('onboarding');
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = true;
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
    currentOnboarding = onboarding;
    whisperCliAvailable = Boolean(onboarding.whisper_cli_available);
    nativeDictationModelReady = Boolean(onboarding.selected_model_exists && whisperCliAvailable);

    if (dictationModelCard) {
      dictationModelCard.hidden = false;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = false;
    }
    if (openSettingsBtn) {
      openSettingsBtn.hidden = false;
    }
    if (dictationDeviceProfileEl) {
      dictationDeviceProfileEl.textContent = describeDeviceProfile(onboarding.device);
    }

    renderDictationModelOptions(onboarding.models, onboarding.selected_model_id);
    applyDictationHotkeyPayload(onboarding);

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
    savedDictationHotkey = null;
    pendingDictationHotkey = '';
    activeHotkeySpec = null;
    isCapturingDictationHotkey = false;
    setSetupScreenMode('onboarding');
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = true;
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

function triggerDictationToggleFromHotkey() {
  const now = Date.now();
  if (now - lastHotkeyToggleAtMs < 180) return;
  lastHotkeyToggleAtMs = now;

  if (isInstallingDictationModel || isDeletingDictationModel) return;
  if (!nativeDictationModelReady) {
    setStatus('Complete setup first, then start dictation.', 'neutral');
    return;
  }

  if (isDictating || isStartingDictation) {
    if (!stopDictationBtn.disabled) {
      stopDictationBtn.click();
    }
    return;
  }

  setAppScreen('dictation');
  if (!startDictationBtn.disabled) {
    startDictationBtn.click();
  }
}

function handleDictationHotkeyEvent(event) {
  if (!isNativeDesktopMode()) return;

  if (isCapturingDictationHotkey) {
    const isCancel = event.key === 'Escape'
      && !event.metaKey
      && !event.ctrlKey
      && !event.altKey
      && !event.shiftKey;
    if (isCancel) {
      event.preventDefault();
      cancelDictationHotkeyCapture();
      return;
    }

    const capturedCombo = buildHotkeyFromEvent(event);
    if (!capturedCombo) return;
    event.preventDefault();

    const parsed = parseHotkeyCombo(capturedCombo);
    if (!parsed.ok) {
      setDictationHotkeyStatus(parsed.error, 'error');
      return;
    }

    pendingDictationHotkey = parsed.display;
    isCapturingDictationHotkey = false;
    setDictationHotkeyStatus(`Pending hotkey: ${parsed.display}. Click "Save Hotkey" to apply.`, 'neutral');
    syncControls();
    return;
  }

  if (!activeHotkeySpec?.ok) return;
  if (!eventMatchesHotkey(event, activeHotkeySpec)) return;

  event.preventDefault();
  triggerDictationToggleFromHotkey();
}

function initDictation() {
  const tauriEventApi = window.__TAURI__?.event || null;
  if (isNativeDesktopMode() && tauriEventApi?.listen) {
    tauriEventApi.listen(DICTATION_HOTKEY_EVENT, () => {
      if (isCapturingDictationHotkey) return;
      triggerDictationToggleFromHotkey();
    }).catch(err => {
      console.error('Failed to register DICTATION_HOTKEY_EVENT listener', err);
      setStatus('Could not register dictation hotkey listener.', 'error');
    });

    tauriEventApi.listen(DICTATION_STATE_EVENT, ({ payload }) => {
      const s = payload?.state ?? 'idle';
      if (s === 'listening') {
        isStartingDictation = false;
        setDictationState(true);
        setUiMode('listening');
        setStatus('Listening\u2026 click Stop to transcribe.', 'live');
      } else if (s === 'processing') {
        isStartingDictation = false;
        setUiMode('loading');
        setStatus('Transcribing captured audio...', 'working');
      } else if (s === 'idle') {
        const didAppendTranscript = appendTranscriptChunk(payload?.transcript);
        if (isDictating || isStartingDictation || didAppendTranscript) {
          isStartingDictation = false;
          setDictationState(false);
          setUiMode('idle');
        }
        if (didAppendTranscript) {
          setStatus('Dictation captured and transcribed.', 'ok');
        }
      } else if (s === 'error') {
        const details = getErrorMessage(payload?.error);
        isStartingDictation = false;
        setDictationState(false);
        setUiMode('error');
        setStatus(`Could not transcribe dictation: ${details}`, 'error');
      }
    }).catch(err => {
      console.error('Failed to register DICTATION_STATE_EVENT listener', err);
      setStatus('Could not register dictation state listener.', 'error');
    });
  }

  document.addEventListener('keydown', handleDictationHotkeyEvent);

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

  if (quickDictationFab) {
    quickDictationFab.addEventListener('click', () => {
      if (isDictating || isStartingDictation) {
        if (!stopDictationBtn.disabled) stopDictationBtn.click();
      } else {
        if (!startDictationBtn.disabled) startDictationBtn.click();
      }
    });
  }

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
    if (recordDictationHotkeyBtn) {
      recordDictationHotkeyBtn.addEventListener('click', beginDictationHotkeyCapture);
    }
    if (saveDictationHotkeyBtn) {
      saveDictationHotkeyBtn.addEventListener('click', async () => {
        const nextValue = String(dictationHotkeyInputEl?.value || pendingDictationHotkey || '').trim();
        await saveDictationHotkey(nextValue);
      });
    }
    if (resetDictationHotkeyBtn) {
      resetDictationHotkeyBtn.addEventListener('click', async () => {
        pendingDictationHotkey = defaultDictationHotkey || DEFAULT_DICTATION_HOTKEY;
        await saveDictationHotkey(pendingDictationHotkey);
      });
    }
    if (clearDictationHotkeyBtn) {
      clearDictationHotkeyBtn.addEventListener('click', clearDictationHotkey);
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

    stopDictationBtn.addEventListener('click', async () => {
      const tauriInvoke = getTauriInvoke();
      if (!tauriInvoke || (!isDictating && !isStartingDictation)) return;

      try {
        const transcriptBeforeStop = finalTranscript;
        setUiMode('loading');
        setStatus('Transcribing captured audio...', 'working');
        const transcript = await tauriInvoke('stop_native_dictation');
        if (finalTranscript === transcriptBeforeStop) {
          const didAppendTranscript = appendTranscriptChunk(transcript);
          if (didAppendTranscript) {
            setUiMode('idle');
            setStatus('Dictation captured and transcribed.', 'ok');
          }
        }
      } catch (error) {
        const details = getErrorMessage(error);
        setUiMode('error');
        setStatus(`Could not stop dictation: ${details}`, 'error');
      } finally {
        isStartingDictation = false;
        setDictationState(false);
      }
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
