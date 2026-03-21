const statusEl = document.getElementById('status');
const onboardingScreen = document.getElementById('onboardingScreen');
const dictationScreen = document.getElementById('dictationScreen');
const onboardingContinueBtn = document.getElementById('onboardingContinue');
const openSettingsBtn = document.getElementById('openSettings');
const setupModeChipEl = document.getElementById('setupModeChip');
const setupTitleEl = document.getElementById('setupTitle');
const setupLeadEl = document.getElementById('setupLead');
const setupStepsEl = document.getElementById('setupSteps');
const startDictationBtn = document.getElementById('startDictation');
const stopDictationBtn = document.getElementById('stopDictation');
const clearTranscriptBtn = document.getElementById('clearTranscript');
const transcriptInput = document.getElementById('transcriptInput');
const dictationWaveformEl = document.getElementById('dictationWaveform');
const dictationWaveformLevelEl = document.getElementById('dictationWaveformLevel');
const dictationHistorySection = document.getElementById('dictationHistorySection');
const dictationHistoryListEl = document.getElementById('dictationHistoryList');
const dictationHistoryEmptyEl = document.getElementById('dictationHistoryEmpty');
const clearDictationHistoryBtn = document.getElementById('clearDictationHistory');
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
const dictationHotkeyPresetsEl = document.getElementById('dictationHotkeyPresets');
const focusedFieldInsertCardEl = document.getElementById('focusedFieldInsertCard');
const focusedFieldInsertToggleEl = document.getElementById('focusedFieldInsertToggle');
const focusedFieldInsertStatusEl = document.getElementById('focusedFieldInsertStatus');
const dictationPermissionsCardEl = document.getElementById('dictationPermissionsCard');
const dictationPermissionSummaryEl = document.getElementById('dictationPermissionSummary');
const dictationPermissionListEl = document.getElementById('dictationPermissionList');
const quickDictationFab = document.getElementById('quickDictationFab');
const dictationWaveformBars = Array.from({ length: 12 }, (_, index) => document.getElementById(`dictationWaveBar${index}`)).filter(Boolean);

const SpeechRecognitionApi = window.SpeechRecognition || window.webkitSpeechRecognition || null;
const DEFAULT_DICTATION_HOTKEY = isMacPlatform() ? 'Fn' : 'CmdOrCtrl+Shift+D';
const HOTKEY_MODIFIER_ORDER = ['CmdOrCtrl', 'Cmd', 'Ctrl', 'Alt', 'Shift', 'Super'];
const DICTATION_HOTKEY_EVENT = 'dictation:hotkey-triggered';
const DICTATION_STATE_EVENT = 'dictation:state-changed';
const DICTATION_AUDIO_LEVEL_EVENT = 'dictation:audio-level';
const NATIVE_HOLD_HOTKEYS = new Set(['Fn', 'F19']);
const MAC_DESKTOP_ONLY_MESSAGE = 'Desktop MVP currently supports macOS only. Current mobile focus is iPhone (iOS).';
const PILL_STATUS_EVENT = 'dicktaint://pill-status';
const DICTATION_HISTORY_LIMIT = 10;
const DICTATION_WAVEFORM_BAR_COUNT = 12;
const HOTKEY_PRESET_OPTIONS = [
  { value: 'Fn', label: 'Hold Fn' },
  { value: 'CmdOrCtrl+Shift+D', label: 'Cmd/Ctrl+Shift+D' },
  { value: 'CmdOrCtrl+Alt+Space', label: 'Cmd/Ctrl+Alt+Space' }
];

let recognition = null;
let currentDraftText = '';
let dictationHistory = [];
let dictationHistorySeq = 0;
let isDictating = false;
let isStartingDictation = false;
let shouldKeepDictating = false;
let restartTimer = null;
let hasMicrophoneAccess = false;
let isInstallingDictationModel = false;
let isDeletingDictationModel = false;
let currentDeviceProfile = null;
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
let dictationTriggerMode = 'disabled';
let dictationTriggerStatus = 'Hotkey disabled.';
let dictationTriggerPermissionHint = '';
let focusedFieldInsertEnabled = false;
let focusedFieldInsertPermissionGranted = false;
let focusedFieldInsertPermissionStatus = 'Focused-field insertion is disabled.';
let isSavingFocusedFieldInsertSetting = false;
let lastHotkeyToggleAtMs = 0;
let nativeHotkeyActionInFlight = false;
let nativeFnHoldActive = false;
let nativeFnStopRequested = false;
let nativeStopRequestInFlight = false;
let pendingNativeStartAfterStop = false;
let pendingNativeStartTrigger = null;
let activeNativeSessionId = null;
let nativeSessionIdToIgnore = null;
let rejectNextNativeAppend = false;
let committedNativeSessionIds = new Set();
let startNativeDesktopDictationOverride = null;
let liveAudioLevel = 0;
let liveAudioBars = defaultLiveAudioBars();

function modelDisplayName(model) {
  return String(model?.display_name || '').replace(/\s+\(Selected\)$/u, '').trim();
}

function clampAudioLevel(value) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return 0;
  return Math.max(0, Math.min(1, numeric));
}

function defaultLiveAudioBars(count = DICTATION_WAVEFORM_BAR_COUNT) {
  return Array.from({ length: count }, (_, index) => {
    const distance = Math.abs(index - ((count - 1) / 2));
    return Math.max(0.08, 0.18 - (distance * 0.016));
  });
}

function fallbackLiveAudioBars(level, count = DICTATION_WAVEFORM_BAR_COUNT) {
  const normalized = clampAudioLevel(level);
  return Array.from({ length: count }, (_, index) => {
    const phase = ((index % 4) + 1) / 4;
    return Math.max(0.08, Math.min(1, (normalized * phase * 0.9) + 0.08));
  });
}

function normalizeLiveAudioBars(rawBars, level, count = DICTATION_WAVEFORM_BAR_COUNT) {
  const source = Array.isArray(rawBars) ? rawBars : [];
  if (!source.length) return fallbackLiveAudioBars(level, count);

  const normalized = [];
  for (let index = 0; index < count; index += 1) {
    const sourceIndex = Math.floor((index * source.length) / count);
    normalized.push(clampAudioLevel(source[sourceIndex]));
  }
  return normalized;
}

function audioStateForLevel(level, mode = document.body?.dataset?.mode || 'idle') {
  if (mode !== 'listening') return mode === 'error' ? 'error' : 'idle';
  if (level < 0.18) return 'low';
  if (level > 0.92) return 'hot';
  return 'ready';
}

function setInlineStyleProperty(target, property, value) {
  if (!target?.style) return;
  if (typeof target.style.setProperty === 'function') {
    target.style.setProperty(property, value);
    return;
  }
  target.style[property] = value;
}

function updateDictationWaveform(level = 0, bars = defaultLiveAudioBars(), mode = document.body?.dataset?.mode || 'idle') {
  liveAudioLevel = clampAudioLevel(level);
  liveAudioBars = normalizeLiveAudioBars(bars, liveAudioLevel, DICTATION_WAVEFORM_BAR_COUNT);

  if (dictationWaveformEl) {
    dictationWaveformEl.dataset.audioState = audioStateForLevel(liveAudioLevel, mode);
    setInlineStyleProperty(dictationWaveformEl, '--live-level', liveAudioLevel.toFixed(3));
  }

  for (let index = 0; index < dictationWaveformBars.length; index += 1) {
    const value = liveAudioBars[index] ?? 0.08;
    setInlineStyleProperty(dictationWaveformBars[index], '--bar-level', value.toFixed(3));
  }

  if (dictationWaveformLevelEl) {
    const state = audioStateForLevel(liveAudioLevel, mode);
    dictationWaveformLevelEl.dataset.audioState = state;
    dictationWaveformLevelEl.textContent = state === 'idle'
      ? 'Mic level: waiting...'
      : (state === 'error'
        ? 'Mic level unavailable.'
        : (state === 'low'
          ? 'Mic level: low'
          : (state === 'hot' ? 'Mic level: hot' : 'Mic level: good')));
  }
}

function resetDictationWaveform(mode = document.body?.dataset?.mode || 'idle') {
  updateDictationWaveform(0, defaultLiveAudioBars(), mode);
}

function isMacPlatform() {
  const source = [
    navigator.userAgentData?.platform,
    navigator.platform,
    navigator.userAgent
  ].filter(Boolean).join(' ');
  return /Mac|iPhone|iPad|iPod/i.test(source);
}

function setDictationHotkeyStatus(message, tone = 'neutral') {
  if (!dictationHotkeyStatusEl) return;
  dictationHotkeyStatusEl.textContent = message;
  dictationHotkeyStatusEl.dataset.tone = tone;
}

function setFocusedFieldInsertStatus(message, tone = 'neutral') {
  if (!focusedFieldInsertStatusEl) return;
  focusedFieldInsertStatusEl.textContent = message;
  focusedFieldInsertStatusEl.dataset.tone = tone;
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
  const code = String(event?.code || '').trim();
  if (/^fn$/i.test(code)) return 'Fn';
  if (/^f19$/i.test(code)) return 'F19';
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

function getSuggestedHotkeyOptions() {
  if (isFocusedMacDesktopMode()) {
    return HOTKEY_PRESET_OPTIONS;
  }
  return HOTKEY_PRESET_OPTIONS.filter((option) => option.value !== 'Fn');
}

function humanizeArchitecture(arch) {
  const value = String(arch || '').trim().toLowerCase();
  if (['aarch64', 'arm64'].includes(value)) return 'Apple silicon';
  if (['x86_64', 'x64', 'amd64'].includes(value)) return 'Intel';
  return arch || 'unknown arch';
}

function describeMachineLabel(device = currentDeviceProfile) {
  if (!device) return 'This Mac';
  if (String(device.os || '').toLowerCase() === 'macos') {
    const arch = humanizeArchitecture(device.architecture);
    return arch === 'Apple silicon' ? 'This Apple silicon Mac' : (arch === 'Intel' ? 'This Intel Mac' : 'This Mac');
  }
  return 'This device';
}

function instructionHotkeyLabel(raw = savedDictationHotkey || pendingDictationHotkey || defaultDictationHotkey) {
  const value = String(raw || '').trim();
  if (!value) return 'a hotkey';
  return value === 'Fn' ? 'Fn / Globe' : value;
}

function isHoldToTalkHotkey() {
  return dictationTriggerMode === 'global-hold'
    || dictationTriggerMode === 'focused-window-hold'
    || activeHotkeySpec?.key === 'Fn';
}

function idlePillMessage() {
  if (!isNativeDesktopMode()) return '';
  if (!isFocusedMacDesktopMode()) return 'Desktop MVP: macOS only';
  if (!currentOnboarding) return 'Checking dictation setup...';
  if (!nativeDictationModelReady) return 'Finish setup in dicktaint';
  if (!savedDictationHotkey) return 'Hotkey disabled - open settings';
  if (dictationTriggerMode === 'focused-window-hold') {
    return `Focus dicktaint, then hold ${instructionHotkeyLabel(savedDictationHotkey)}`;
  }
  if (isHoldToTalkHotkey()) {
    return `Hold ${instructionHotkeyLabel(savedDictationHotkey)} to dictate`;
  }
  return `Press ${instructionHotkeyLabel(savedDictationHotkey)} to dictate`;
}

function listeningStatusForTrigger(trigger) {
  if (trigger === 'hotkey-hold' || isHoldToTalkHotkey()) {
    return `Listening... release ${instructionHotkeyLabel(savedDictationHotkey)} to stop and transcribe.`;
  }
  if (trigger === 'hotkey') {
    return `Listening... press ${instructionHotkeyLabel(savedDictationHotkey)} again to stop and transcribe.`;
  }
  return 'Listening... click Stop to transcribe.';
}

function completedStatusForTrigger(trigger) {
  if (trigger === 'hotkey-hold' || isHoldToTalkHotkey()) {
    return `Dictation captured from ${instructionHotkeyLabel(savedDictationHotkey)} hold and transcribed.`;
  }
  if (trigger === 'hotkey') {
    return `Dictation captured from ${instructionHotkeyLabel(savedDictationHotkey)} and transcribed.`;
  }
  return 'Dictation captured and transcribed.';
}

function renderDictationHotkeyPresets() {
  if (!dictationHotkeyPresetsEl) return;
  dictationHotkeyPresetsEl.innerHTML = '';
  const activeValue = String(pendingDictationHotkey || savedDictationHotkey || '').trim();
  const normalizedActive = parseHotkeyCombo(activeValue);

  for (const option of getSuggestedHotkeyOptions()) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'ghost quiet preset-chip';
    button.textContent = option.label;
    button.dataset.hotkeyPreset = option.value;
    if (normalizedActive.ok && normalizedActive.display === option.value) {
      button.className += ' is-active';
    }
    dictationHotkeyPresetsEl.appendChild(button);
  }
}

function renderPermissionGuidance() {
  if (!dictationPermissionsCardEl || !dictationPermissionSummaryEl || !dictationPermissionListEl) return;

  dictationPermissionsCardEl.hidden = !isNativeDesktopMode();
  if (!isNativeDesktopMode()) return;

  const machineLabel = describeMachineLabel();
  if (!isFocusedMacDesktopMode()) {
    dictationPermissionSummaryEl.textContent = `${machineLabel} is outside the supported macOS desktop path.`;
    dictationPermissionListEl.innerHTML = '';
    return;
  }

  dictationPermissionSummaryEl.textContent = nativeDictationModelReady
    ? `${machineLabel} is ready. ${dictationTriggerStatus}`
    : `${machineLabel} still needs local setup. ${dictationTriggerStatus}`;

  const items = [
    {
      tone: nativeDictationModelReady ? 'ok' : 'neutral',
      text: 'Microphone: macOS asks the first time you start dictation. If audio fails later, relaunch after changing permission.'
    }
  ];

  if (savedDictationHotkey) {
    if (dictationTriggerMode === 'focused-window-hold') {
      items.push({
        tone: 'warn',
        text: dictationTriggerPermissionHint || 'Input Monitoring is required for global Fn hold-to-talk. Without it, Fn only works while dicktaint is focused.'
      });
    } else if (dictationTriggerMode === 'global-hold') {
      items.push({
        tone: 'ok',
        text: 'Input Monitoring: global Fn hold-to-talk is active for this app while it is running.'
      });
    } else {
      items.push({
        tone: 'ok',
        text: `Hotkey: ${instructionHotkeyLabel(savedDictationHotkey)} is registered globally while dicktaint is running.`
      });
    }
  } else {
    items.push({
      tone: 'neutral',
      text: 'Hotkey: disabled. Open Settings if you want a system-wide trigger again.'
    });
  }

  items.push({
    tone: focusedFieldInsertEnabled
      ? (focusedFieldInsertPermissionGranted ? 'ok' : 'warn')
      : 'neutral',
    text: focusedFieldInsertEnabled
      ? (focusedFieldInsertPermissionStatus
        || 'Accessibility is required for Dictate Into Focused Field.')
      : 'Accessibility is only needed if you enable Dictate Into Focused Field.'
  });

  dictationPermissionListEl.innerHTML = '';
  for (const item of items) {
    const li = document.createElement('li');
    li.dataset.tone = item.tone;
    li.textContent = item.text;
    dictationPermissionListEl.appendChild(li);
  }
}

function syncDictationHotkeyUi() {
  const nativeDesktop = isNativeDesktopMode();
  if (dictationHotkeyCardEl) dictationHotkeyCardEl.hidden = !nativeDesktop;
  if (focusedFieldInsertCardEl) focusedFieldInsertCardEl.hidden = !nativeDesktop;
  if (dictationPermissionsCardEl) dictationPermissionsCardEl.hidden = !nativeDesktop;
  if (!nativeDesktop) return;

  if (dictationHotkeyInputEl) {
    dictationHotkeyInputEl.value = pendingDictationHotkey || savedDictationHotkey || '';
    dictationHotkeyInputEl.placeholder = defaultDictationHotkey || DEFAULT_DICTATION_HOTKEY;
  }
  if (recordDictationHotkeyBtn) {
    recordDictationHotkeyBtn.textContent = isCapturingDictationHotkey ? 'Press Keys...' : 'Record';
  }
  renderDictationHotkeyPresets();
  renderPermissionGuidance();
}

function getTauriInvoke() {
  return window.__TAURI__?.core?.invoke
    || window.__TAURI__?.tauri?.invoke
    || null;
}

function getTauriEventApi() {
  return window.__TAURI__?.event || null;
}

function detectDesktopOs() {
  const source = [
    currentDeviceProfile?.os,
    navigator.userAgentData?.platform,
    navigator.platform,
    navigator.userAgent
  ].filter(Boolean).join(' ');

  if (/mac|darwin/i.test(source)) return 'macos';
  if (/win/i.test(source)) return 'windows';
  if (/linux|x11/i.test(source)) return 'linux';
  return 'unknown';
}

function isMobileUserAgent() {
  if (navigator.userAgentData?.mobile) return true;
  const ua = navigator.userAgent || '';
  return /Android|iPhone|iPad|iPod/i.test(ua);
}

function isNativeDesktopMode() {
  return Boolean(getTauriInvoke()) && !isMobileUserAgent();
}

function isFocusedMacDesktopMode() {
  return isNativeDesktopMode() && detectDesktopOs() === 'macos';
}

function shouldUseTauriCommands() {
  return isFocusedMacDesktopMode();
}

function setUiMode(mode) {
  document.body.dataset.mode = mode;
  if (mode === 'listening') {
    updateDictationWaveform(liveAudioLevel, liveAudioBars, mode);
  } else {
    resetDictationWaveform(mode);
  }
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
  if (!isFocusedMacDesktopMode()) return 'Desktop MVP: macOS only';
  const normalized = String(message || '').toLowerCase();
  const hotkeyLabel = instructionHotkeyLabel(savedDictationHotkey);

  if (tone === 'live') {
    if (isHoldToTalkHotkey()) {
      return `Listening - release ${hotkeyLabel}`;
    }
    return `Listening - press ${hotkeyLabel} again`;
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
    return idlePillMessage();
  }
  if (tone === 'error') {
    return 'Dictation error - check status';
  }
  return idlePillMessage();
}

function syncHotkeyPillForStatus(message, tone = 'neutral') {
  const visible = isNativeDesktopMode() && isFocusedMacDesktopMode();
  if (!visible) {
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

  startDictationBtn.disabled = (
    lockControls
    || !hasCaptureSupport
    || isDictating
    || isStartingDictation
    || nativeStopRequestInFlight
    || dictationModelMissing
  );
  stopDictationBtn.disabled = (
    lockControls
    || !hasCaptureSupport
    || nativeStopRequestInFlight
    || (!isDictating && !isStartingDictation)
  );
  clearTranscriptBtn.disabled = lockControls || nativeStopRequestInFlight;

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
    onboardingContinueBtn.disabled = lockControls || (setupScreenMode === 'onboarding' && !setupReady);
    onboardingContinueBtn.textContent = setupScreenMode === 'settings'
      ? 'Done'
      : (setupReady ? 'Start Dictation' : 'Complete Setup to Continue');
  }
  if (openSettingsBtn) {
    openSettingsBtn.disabled = lockControls || !isFocusedMacDesktopMode();
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
  if (focusedFieldInsertToggleEl) {
    focusedFieldInsertToggleEl.disabled = hotkeyDisabled || isSavingFocusedFieldInsertSetting;
    focusedFieldInsertToggleEl.checked = focusedFieldInsertEnabled;
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

function normalizeNativeDictationError(text) {
  return String(text || '').trim().toLowerCase();
}

function isStartConflictDictationError(text) {
  const normalized = normalizeNativeDictationError(text);
  return normalized.includes('dictation already running');
}

function isStopNoopDictationError(text) {
  const normalized = normalizeNativeDictationError(text);
  return normalized.includes('dictation is not running');
}

function nextDictationHistoryId() {
  dictationHistorySeq += 1;
  return `dict-${Date.now()}-${dictationHistorySeq}`;
}

function pushDictationHistory(chunk, source = 'native') {
  const trimmed = String(chunk || '').trim();
  if (!trimmed) return;
  dictationHistory = [
    {
      id: nextDictationHistoryId(),
      text: trimmed,
      source,
      createdAt: new Date().toISOString()
    },
    ...dictationHistory
  ].slice(0, DICTATION_HISTORY_LIMIT);
  renderDictationHistory();
}

/**
 * Attempts to record a native session as committed.
 * Returns false when a duplicate session id is detected; null/empty ids return true.
 */
function tryCommitNativeSession(sessionId) {
  if (!sessionId) return true;
  if (committedNativeSessionIds.has(sessionId)) return false;
  committedNativeSessionIds.add(sessionId);
  if (committedNativeSessionIds.size > 64) {
    const keep = Array.from(committedNativeSessionIds).slice(-32);
    committedNativeSessionIds = new Set(keep);
  }
  return true;
}

function normalizeNativeSessionId(sessionId) {
  if (sessionId === null || sessionId === undefined) return null;
  const normalized = String(sessionId).trim();
  return normalized || null;
}

function queueNativeStartAfterCurrentStop(trigger = 'hotkey') {
  pendingNativeStartAfterStop = true;
  pendingNativeStartTrigger = trigger;
}

async function maybeStartQueuedNativeDictation() {
  if (!pendingNativeStartAfterStop) return;
  if (isDictating || isStartingDictation || nativeStopRequestInFlight) return;

  const trigger = pendingNativeStartTrigger || 'hotkey';
  pendingNativeStartAfterStop = false;
  pendingNativeStartTrigger = null;
  const startFn = startNativeDesktopDictationOverride || startNativeDesktopDictation;
  try {
    await startFn(trigger);
  } catch (error) {
    const details = getErrorMessage(error);
    isStartingDictation = false;
    setDictationState(false);
    setUiMode('error');
    setStatus(`Could not start dictation: ${details}`, 'error');
    console.error('Could not start queued dictation', error);
  }
}

function setDraftTranscriptText(text) {
  currentDraftText = String(text || '').trim();
  transcriptInput.value = currentDraftText;
}

function appendToDraftTranscript(text) {
  const trimmed = String(text || '').trim();
  if (!trimmed) return false;
  currentDraftText = `${currentDraftText} ${trimmed}`.trim();
  transcriptInput.value = currentDraftText;
  return true;
}

function findDictationHistoryEntry(historyId) {
  const id = String(historyId || '').trim();
  if (!id) return null;
  return dictationHistory.find((entry) => entry.id === id) || null;
}

function formatHistoryTimestamp(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

function historySourceLabel(source) {
  const value = String(source || '').trim().toLowerCase();
  if (value === 'web') return 'WEB';
  if (value === 'native' || value === 'native-event') return 'DESKTOP';
  return 'DICTATION';
}

async function copyTextToClipboard(text) {
  const trimmed = String(text || '').trim();
  if (!trimmed) return false;

  const tauriClipboard = window.__TAURI__?.clipboard
    || window.__TAURI__?.plugins?.clipboard
    || null;
  if (typeof tauriClipboard?.writeText === 'function') {
    await tauriClipboard.writeText(trimmed);
    return true;
  }

  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(trimmed);
    return true;
  }

  console.warn('Clipboard fallback unavailable: no tauri clipboard API or navigator.clipboard.writeText.');
  return false;
}

async function runDictationHistoryAction(historyAction, historyId) {
  const action = String(historyAction || '').trim();
  const id = String(historyId || '').trim();
  const entry = findDictationHistoryEntry(id);
  if (!entry) {
    setStatus('That history entry is no longer available.', 'error');
    return false;
  }

  if (action === 'reinsert') {
    if (appendToDraftTranscript(entry.text)) {
      transcriptInput.focus();
      setStatus('Reinserted previous dictation into transcript.', 'ok');
      return true;
    }
    return false;
  }

  if (action === 'copy') {
    try {
      const copied = await copyTextToClipboard(entry.text);
      if (copied) {
        setStatus('Copied dictation entry to clipboard.', 'ok');
      } else {
        setStatus('Could not copy dictation entry to clipboard.', 'error');
      }
      return copied;
    } catch (error) {
      const details = getErrorMessage(error);
      setStatus(`Could not copy dictation entry: ${details}`, 'error');
      return false;
    }
  }

  setStatus(`Unknown history action: ${action || '(empty)'}.`, 'error');
  return false;
}

function renderDictationHistory() {
  if (!dictationHistorySection || !dictationHistoryListEl || !dictationHistoryEmptyEl) return;
  dictationHistorySection.hidden = false;
  const hasHistory = dictationHistory.length > 0;
  dictationHistoryEmptyEl.hidden = hasHistory;
  if (clearDictationHistoryBtn) clearDictationHistoryBtn.disabled = !hasHistory;

  dictationHistoryListEl.innerHTML = '';
  if (!hasHistory) return;

  for (const entry of dictationHistory) {
    const item = document.createElement('li');
    item.className = 'dictation-history-item';

    const text = document.createElement('p');
    text.className = 'dictation-history-text';
    text.textContent = entry.text;

    const meta = document.createElement('p');
    meta.className = 'dictation-history-meta';
    const stamp = formatHistoryTimestamp(entry.createdAt);
    const source = historySourceLabel(entry.source);
    meta.textContent = stamp ? `${stamp} • ${source}` : source;

    const actions = document.createElement('div');
    actions.className = 'dictation-history-actions';

    const reuseBtn = document.createElement('button');
    reuseBtn.type = 'button';
    reuseBtn.className = 'ghost quiet';
    reuseBtn.textContent = 'Reinsert';
    reuseBtn.dataset.historyAction = 'reinsert';
    reuseBtn.dataset.historyId = entry.id;

    const copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'ghost quiet';
    copyBtn.textContent = 'Copy';
    copyBtn.dataset.historyAction = 'copy';
    copyBtn.dataset.historyId = entry.id;

    actions.appendChild(reuseBtn);
    actions.appendChild(copyBtn);
    item.appendChild(text);
    item.appendChild(meta);
    item.appendChild(actions);
    dictationHistoryListEl.appendChild(item);
  }
}

function appendTranscriptChunk(chunk, { source = 'native', nativeSessionId = null } = {}) {
  const trimmed = String(chunk || '').trim();
  if (!trimmed) return false;
  const isNativeSource = source === 'native' || source === 'native-event';
  if (isNativeSource && rejectNextNativeAppend) {
    return false;
  }
  if (isNativeSource && nativeSessionIdToIgnore && (nativeSessionId === null || nativeSessionId === nativeSessionIdToIgnore)) {
    return false;
  }
  if (!tryCommitNativeSession(nativeSessionId)) return false;
  appendToDraftTranscript(trimmed);
  rejectNextNativeAppend = false;
  pushDictationHistory(trimmed, source);
  void maybeInsertTranscriptIntoFocusedField(trimmed);
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
  const machine = describeMachineLabel(device).replace(/^This /u, '');
  return `${machine} • ${ram} GB RAM • ${cores} logical CPU cores • ${device.os || 'unknown os'}`;
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
  dictationTriggerMode = String(
    payload?.dictation_trigger_mode
      || payload?.trigger_mode
      || 'disabled'
  ).trim() || 'disabled';
  dictationTriggerStatus = String(
    payload?.dictation_trigger_status
      || payload?.trigger_status
      || 'Hotkey disabled.'
  ).trim() || 'Hotkey disabled.';
  dictationTriggerPermissionHint = String(
    payload?.dictation_trigger_permission_hint
      || payload?.trigger_permission_hint
      || ''
  ).trim();

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
  setDictationHotkeyStatus(`Current hotkey: ${parsedTrigger.display}. ${dictationTriggerStatus}`, 'ok');
  syncControls();
}

function applyFocusedFieldInsertPayload(payload) {
  const enabled = Boolean(
    payload?.focused_field_insert_enabled
      ?? payload?.focusedFieldInsertEnabled
      ?? payload?.enabled
  );
  focusedFieldInsertPermissionGranted = Boolean(
    payload?.focused_field_insert_permission_granted
      ?? payload?.focusedFieldInsertPermissionGranted
      ?? payload?.permission_granted
  );
  focusedFieldInsertPermissionStatus = String(
    (
      payload?.focused_field_insert_permission_status
      ?? payload?.focusedFieldInsertPermissionStatus
      ?? payload?.permission_status
    ) || ''
  ).trim();
  focusedFieldInsertEnabled = enabled;

  if (focusedFieldInsertToggleEl) {
    focusedFieldInsertToggleEl.checked = enabled;
  }

  if (enabled && focusedFieldInsertPermissionGranted) {
    setFocusedFieldInsertStatus(
      focusedFieldInsertPermissionStatus || 'Focused-field insertion is enabled.',
      'ok'
    );
  } else if (enabled) {
    setFocusedFieldInsertStatus(
      focusedFieldInsertPermissionStatus || 'Focused-field insertion needs Accessibility permission.',
      'error'
    );
  } else {
    setFocusedFieldInsertStatus(
      focusedFieldInsertPermissionStatus || 'Focused-field insertion is disabled.',
      'neutral'
    );
  }
  syncControls();
}

async function saveFocusedFieldInsertSetting(enabled) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  try {
    isSavingFocusedFieldInsertSetting = true;
    syncControls();
    const payload = await tauriInvoke('set_focused_field_insert_enabled', { enabled: Boolean(enabled) });
    applyFocusedFieldInsertPayload(payload);
    setStatus(
      focusedFieldInsertEnabled
        ? (focusedFieldInsertPermissionGranted
          ? 'Focused-field insertion enabled.'
          : focusedFieldInsertPermissionStatus)
        : 'Focused-field insertion disabled.',
      focusedFieldInsertEnabled && !focusedFieldInsertPermissionGranted ? 'error' : 'ok'
    );
  } catch (error) {
    const details = getErrorMessage(error);
    if (focusedFieldInsertToggleEl) {
      focusedFieldInsertToggleEl.checked = focusedFieldInsertEnabled;
    }
    setFocusedFieldInsertStatus(`Could not save setting: ${details}`, 'error');
    setStatus(`Could not save focused-field insertion setting: ${details}`, 'error');
  } finally {
    isSavingFocusedFieldInsertSetting = false;
    syncControls();
  }
}

async function maybeInsertTranscriptIntoFocusedField(chunk) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode() || !focusedFieldInsertEnabled) return;
  if (typeof document.hasFocus === 'function' && document.hasFocus()) return;

  const trimmed = String(chunk || '').trim();
  if (!trimmed) return;

  try {
    await tauriInvoke('insert_text_into_focused_field', { text: trimmed });
  } catch (error) {
    const details = getErrorMessage(error);
    setFocusedFieldInsertStatus(`Insert failed: ${details}`, 'error');
    setStatus(`Transcript captured, but focused-field insert failed: ${details}`, 'error');
  }
}

function beginDictationHotkeyCapture() {
  if (!isNativeDesktopMode()) return;
  isCapturingDictationHotkey = true;
  setDictationHotkeyStatus('Press your desired key combo now. Press Escape to cancel. For Fn/Globe, tap and release it once.', 'neutral');
  syncControls();
}

function cancelDictationHotkeyCapture() {
  if (!isCapturingDictationHotkey) return;
  isCapturingDictationHotkey = false;
  if (savedDictationHotkey) {
    setDictationHotkeyStatus(`Current hotkey: ${savedDictationHotkey}. ${dictationTriggerStatus}`, 'ok');
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
    currentDeviceProfile = null;
    savedDictationHotkey = null;
    pendingDictationHotkey = '';
    activeHotkeySpec = null;
    isCapturingDictationHotkey = false;
    dictationTriggerMode = 'disabled';
    dictationTriggerStatus = 'Hotkey disabled.';
    dictationTriggerPermissionHint = '';
    focusedFieldInsertEnabled = false;
    focusedFieldInsertPermissionGranted = false;
    focusedFieldInsertPermissionStatus = 'Focused-field insertion is disabled.';
    isSavingFocusedFieldInsertSetting = false;
    setFocusedFieldInsertStatus('Focused-field insertion is disabled.', 'neutral');
    setSetupScreenMode('onboarding');
    if (dictationModelCard) {
      dictationModelCard.hidden = true;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = true;
    }
    if (focusedFieldInsertCardEl) {
      focusedFieldInsertCardEl.hidden = true;
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
    currentDeviceProfile = { os: detectDesktopOs(), architecture: navigator.platform || '' };
    focusedFieldInsertEnabled = false;
    focusedFieldInsertPermissionGranted = false;
    focusedFieldInsertPermissionStatus = 'Focused-field insertion is unavailable on this platform.';
    isSavingFocusedFieldInsertSetting = false;
    dictationTriggerMode = 'disabled';
    dictationTriggerStatus = 'Hotkey unavailable on this platform.';
    dictationTriggerPermissionHint = '';
    setFocusedFieldInsertStatus('Focused-field insertion is unavailable on this platform.', 'neutral');
    setSetupScreenMode('onboarding');
    if (dictationModelCard) {
      dictationModelCard.hidden = true;
    }
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    if (focusedFieldInsertCardEl) {
      focusedFieldInsertCardEl.hidden = true;
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
    currentDeviceProfile = null;
    savedDictationHotkey = null;
    pendingDictationHotkey = '';
    activeHotkeySpec = null;
    isCapturingDictationHotkey = false;
    dictationTriggerMode = 'disabled';
    dictationTriggerStatus = 'Desktop bridge offline.';
    dictationTriggerPermissionHint = '';
    focusedFieldInsertEnabled = false;
    focusedFieldInsertPermissionGranted = false;
    focusedFieldInsertPermissionStatus = 'Focused-field insertion is unavailable while desktop bridge is offline.';
    isSavingFocusedFieldInsertSetting = false;
    setFocusedFieldInsertStatus('Focused-field insertion is unavailable while desktop bridge is offline.', 'neutral');
    setSetupScreenMode('onboarding');
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = true;
    }
    if (focusedFieldInsertCardEl) {
      focusedFieldInsertCardEl.hidden = true;
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
    currentDeviceProfile = onboarding.device || null;
    whisperCliAvailable = Boolean(onboarding.whisper_cli_available);
    nativeDictationModelReady = Boolean(onboarding.selected_model_exists && whisperCliAvailable);

    if (dictationModelCard) {
      dictationModelCard.hidden = false;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = false;
    }
    if (focusedFieldInsertCardEl) {
      focusedFieldInsertCardEl.hidden = false;
    }
    if (openSettingsBtn) {
      openSettingsBtn.hidden = false;
    }
    if (dictationDeviceProfileEl) {
      dictationDeviceProfileEl.textContent = describeDeviceProfile(onboarding.device);
    }

    renderDictationModelOptions(onboarding.models, onboarding.selected_model_id);
    applyDictationHotkeyPayload(onboarding);
    applyFocusedFieldInsertPayload(onboarding);

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
    currentDeviceProfile = null;
    savedDictationHotkey = null;
    pendingDictationHotkey = '';
    activeHotkeySpec = null;
    isCapturingDictationHotkey = false;
    dictationTriggerMode = 'disabled';
    dictationTriggerStatus = 'Could not load hotkey state.';
    dictationTriggerPermissionHint = '';
    focusedFieldInsertEnabled = false;
    focusedFieldInsertPermissionGranted = false;
    focusedFieldInsertPermissionStatus = 'Focused-field insertion is unavailable while setup is loading.';
    isSavingFocusedFieldInsertSetting = false;
    setFocusedFieldInsertStatus('Focused-field insertion is unavailable while setup is loading.', 'neutral');
    setSetupScreenMode('onboarding');
    if (openSettingsBtn) {
      openSettingsBtn.hidden = true;
    }
    if (dictationHotkeyCardEl) {
      dictationHotkeyCardEl.hidden = true;
    }
    if (focusedFieldInsertCardEl) {
      focusedFieldInsertCardEl.hidden = true;
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

async function startNativeDesktopDictation(trigger = 'button', shouldRetryOnConflict = true) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;
  if (nativeStopRequestInFlight) {
    queueNativeStartAfterCurrentStop(trigger);
    setStatus('Finishing previous dictation... next one will start automatically.', 'working');
    return;
  }
  if (isDictating || isStartingDictation) return;

  if (!nativeDictationModelReady) {
    setStatus('Complete setup first, then start dictation.', 'neutral');
    return;
  }

  try {
    isStartingDictation = true;
    nativeSessionIdToIgnore = null;
    rejectNextNativeAppend = false;
    syncControls();
    setUiMode('loading');
    setStatus('Opening microphone...', 'working');
    activeNativeSessionId = null;
    await tauriInvoke('start_native_dictation');
    isStartingDictation = false;
    setDictationState(true);
    setUiMode('listening');
    setStatus(listeningStatusForTrigger(trigger), 'live');
  } catch (error) {
    const details = getErrorMessage(error);
    if (shouldRetryOnConflict && isStartConflictDictationError(details)) {
      setStatus('Recovering from stale dictation state...', 'working');
      nativeSessionIdToIgnore = null;
      rejectNextNativeAppend = false;
      activeNativeSessionId = null;
      isStartingDictation = false;
      setDictationState(false);
      try {
        await tauriInvoke('cancel_native_dictation');
        return startNativeDesktopDictation(trigger, false);
      } catch (recoverError) {
        isStartingDictation = false;
        setDictationState(false);
        setUiMode('error');
        setStatus(`Could not recover dictation session: ${getErrorMessage(recoverError)}`, 'error');
        return;
      }
    }

    isStartingDictation = false;
    setDictationState(false);
    activeNativeSessionId = null;
    setUiMode('error');
    setStatus(`Could not start dictation: ${details}`, 'error');
  }
}

async function stopNativeDesktopDictation(trigger = 'button') {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || (!isDictating && !isStartingDictation)) return;
  if (nativeStopRequestInFlight) return;

  nativeStopRequestInFlight = true;
  const sessionId = activeNativeSessionId;
  try {
    setUiMode('loading');
    setStatus('Transcribing captured audio...', 'working');
    const transcript = await tauriInvoke('stop_native_dictation');
    const didAppendTranscript = appendTranscriptChunk(transcript, {
      source: 'native',
      nativeSessionId: sessionId
    });
    setUiMode('idle');
    if (didAppendTranscript) {
      setStatus(completedStatusForTrigger(trigger), 'ok');
    } else {
      setStatus('No new dictation content to save.', 'neutral');
    }
  } catch (error) {
    const details = getErrorMessage(error);
    if (isStopNoopDictationError(details)) {
      setUiMode('idle');
      activeNativeSessionId = null;
      setStatus('No active dictation session to stop.', 'neutral');
      return;
    }
    setUiMode('error');
    setStatus(`Could not stop dictation: ${details}`, 'error');
  } finally {
    nativeStopRequestInFlight = false;
    isStartingDictation = false;
    setDictationState(false);
    void maybeStartQueuedNativeDictation();
  }
}

function isNativeHoldHotkeyEvent(event) {
  const key = eventKeyToken(event);
  if (key && NATIVE_HOLD_HOTKEYS.has(key)) return true;

  const keyName = String(event?.key || '').trim().toLowerCase();
  if (keyName === 'fn' || keyName === 'f19') return true;
  return keyName === 'unidentified' && Boolean(event.getModifierState?.('Fn'));
}

function applyNativeFnHoldState(pressed) {
  if (!isFocusedMacDesktopMode()) return;
  const nextPressed = Boolean(pressed);

  if (nextPressed) {
    nativeFnHoldActive = true;
    nativeFnStopRequested = false;
    if (nativeStopRequestInFlight) {
      queueNativeStartAfterCurrentStop('hotkey-hold');
      setHotkeyPill('finishing previous dictation...', 'working', true);
      return;
    }

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
    // Release can arrive while startup is in-flight; defer stop until capture is live.
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

function handleNativeHoldKeydown(event) {
  if (!isFocusedMacDesktopMode()) return;
  if (dictationTriggerMode !== 'focused-window-hold') return;
  if (event.repeat) return;
  if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
  if (!isNativeHoldHotkeyEvent(event)) return;
  if (activeHotkeySpec?.ok && activeHotkeySpec.key !== 'Fn') return;

  event.preventDefault();
  event.stopPropagation();
  applyNativeFnHoldState(true);
}

function handleNativeHoldKeyup(event) {
  if (!isFocusedMacDesktopMode()) return;
  if (dictationTriggerMode !== 'focused-window-hold') return;
  if (!isNativeHoldHotkeyEvent(event)) return;
  if (activeHotkeySpec?.ok && activeHotkeySpec.key !== 'Fn') return;

  event.preventDefault();
  event.stopPropagation();
  applyNativeFnHoldState(false);
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
  if (nativeStopRequestInFlight) {
    queueNativeStartAfterCurrentStop('hotkey');
    setStatus('Finishing previous dictation... next one will start automatically.', 'working');
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

function maybeCaptureDictationHotkeyEvent(event) {
  if (!isCapturingDictationHotkey) return false;

  const isCancel = event.type === 'keydown'
    && event.key === 'Escape'
    && !event.metaKey
    && !event.ctrlKey
    && !event.altKey
    && !event.shiftKey;
  if (isCancel) {
    event.preventDefault();
    cancelDictationHotkeyCapture();
    return true;
  }

  const capturedCombo = buildHotkeyFromEvent(event);
  if (!capturedCombo) return false;

  if (event.type === 'keyup' && capturedCombo !== 'Fn') {
    return false;
  }

  event.preventDefault();
  const parsed = parseHotkeyCombo(capturedCombo);
  if (!parsed.ok) {
    setDictationHotkeyStatus(parsed.error, 'error');
    return true;
  }

  pendingDictationHotkey = parsed.display;
  isCapturingDictationHotkey = false;
  setDictationHotkeyStatus(`Pending hotkey: ${parsed.display}. Click "Save Hotkey" to apply.`, 'neutral');
  syncControls();
  return true;
}

function handleDictationHotkeyEvent(event) {
  if (!isNativeDesktopMode()) return;

  if (maybeCaptureDictationHotkeyEvent(event)) {
    return;
  }
}

function handleNativeDictationStatePayload(payload) {
  const s = payload?.state ?? 'idle';
  const payloadSessionId = normalizeNativeSessionId(payload?.session_id);
  const sessionMatchesCurrent = !payloadSessionId
    || !activeNativeSessionId
    || payloadSessionId === activeNativeSessionId;

  if (s === 'listening') {
    activeNativeSessionId = payloadSessionId || activeNativeSessionId;
    isStartingDictation = false;
    setDictationState(true);
    setUiMode('listening');
    setStatus('Listening\u2026 click Stop to transcribe.', 'live');
    return;
  }

  if (s === 'processing') {
    if (payloadSessionId && !activeNativeSessionId) {
      activeNativeSessionId = payloadSessionId;
    }
    isStartingDictation = false;
    setUiMode('loading');
    setStatus('Transcribing captured audio...', 'working');
    return;
  }

  if (s === 'idle') {
    const transcriptSessionId = payloadSessionId || activeNativeSessionId;
    const didAppendTranscript = nativeStopRequestInFlight
      ? false
      : appendTranscriptChunk(payload?.transcript, {
        source: 'native-event',
        nativeSessionId: transcriptSessionId
      });

    if (sessionMatchesCurrent && (isDictating || isStartingDictation || didAppendTranscript)) {
      isStartingDictation = false;
      setDictationState(false);
      setUiMode('idle');
    }
    if (sessionMatchesCurrent) {
      activeNativeSessionId = null;
    }
    if (didAppendTranscript && sessionMatchesCurrent) {
      setStatus('Dictation captured and transcribed.', 'ok');
    }
    return;
  }

  if (s === 'error') {
    const details = getErrorMessage(payload?.error);
    if (sessionMatchesCurrent) {
      activeNativeSessionId = null;
      nativeStopRequestInFlight = false;
      isStartingDictation = false;
      setDictationState(false);
      setUiMode('error');
      setStatus(`Could not transcribe dictation: ${details}`, 'error');
      void maybeStartQueuedNativeDictation();
    }
  }
}

function handleNativeDictationAudioLevelPayload(payload) {
  const payloadSessionId = normalizeNativeSessionId(payload?.session_id);
  if (payloadSessionId && activeNativeSessionId && payloadSessionId !== activeNativeSessionId) {
    return;
  }

  const level = clampAudioLevel(payload?.level);
  const bars = normalizeLiveAudioBars(payload?.bars, level, DICTATION_WAVEFORM_BAR_COUNT);
  updateDictationWaveform(level, bars, 'listening');
}

function initDictation() {
  const tauriEventApi = window.__TAURI__?.event || null;
  if (isNativeDesktopMode() && tauriEventApi?.listen) {
    tauriEventApi.listen(DICTATION_HOTKEY_EVENT, ({ payload }) => {
      if (isCapturingDictationHotkey) return;
      if (dictationTriggerMode !== 'focused-window-hold') return;
      if (activeHotkeySpec?.ok && activeHotkeySpec.key === 'Fn') {
        applyNativeFnHoldState(payload?.pressed !== false);
        return;
      }
      if (payload?.pressed === false) return;
      triggerDictationToggleFromHotkey();
    }).catch(err => {
      console.error('Failed to register DICTATION_HOTKEY_EVENT listener', err);
      setStatus('Could not register dictation hotkey listener.', 'error');
    });

    tauriEventApi.listen(DICTATION_STATE_EVENT, ({ payload }) => {
      handleNativeDictationStatePayload(payload);
    }).catch(err => {
      console.error('Failed to register DICTATION_STATE_EVENT listener', err);
      setStatus('Could not register dictation state listener.', 'error');
    });

    tauriEventApi.listen(DICTATION_AUDIO_LEVEL_EVENT, ({ payload }) => {
      handleNativeDictationAudioLevelPayload(payload);
    }).catch(err => {
      console.error('Failed to register DICTATION_AUDIO_LEVEL_EVENT listener', err);
    });
  }

  document.addEventListener('keydown', handleDictationHotkeyEvent);
  document.addEventListener('keyup', handleDictationHotkeyEvent);

  clearTranscriptBtn.addEventListener('click', () => {
    const tauriInvoke = shouldUseTauriCommands() ? getTauriInvoke() : null;
    if (tauriInvoke) {
      tauriInvoke('cancel_native_dictation').catch(() => {});
    }

    shouldKeepDictating = false;
    isStartingDictation = false;
    pendingNativeStartAfterStop = false;
    pendingNativeStartTrigger = null;
    nativeSessionIdToIgnore = activeNativeSessionId;
    rejectNextNativeAppend = false;
    activeNativeSessionId = null;
    setDictationState(false);
    syncControls();
    clearRestartTimer();
    setDraftTranscriptText('');
    setUiMode('idle');
    setStatus('Transcript cleared. Recent dictation history is still available in app state.', 'neutral');
  });

  if (clearDictationHistoryBtn) {
    clearDictationHistoryBtn.addEventListener('click', () => {
      dictationHistory = [];
      renderDictationHistory();
      setStatus('Recent dictation history cleared.', 'neutral');
    });
  }

  if (dictationHistoryListEl) {
    dictationHistoryListEl.addEventListener('click', (event) => {
      const target = event.target instanceof Element
        ? event.target.closest('button[data-history-action][data-history-id]')
        : null;
      if (!target) return;

      const historyAction = String(target.dataset.historyAction || '').trim();
      const historyId = String(target.dataset.historyId || '').trim();
      void runDictationHistoryAction(historyAction, historyId);
    });
  }

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
    currentDraftText = transcriptInput.value;
  });

  renderDictationHistory();

  if (onboardingContinueBtn) {
    onboardingContinueBtn.addEventListener('click', () => {
      if (setupScreenMode === 'onboarding' && isFocusedMacDesktopMode() && !nativeDictationModelReady) {
        setStatus('Complete setup first, then start dictation.', 'neutral');
        return;
      }
      setAppScreen('dictation');
      setStatus(setupScreenMode === 'settings' ? 'Settings closed.' : 'Dictation ready.', 'ok');
    });
  }

  if (openSettingsBtn) {
    openSettingsBtn.addEventListener('click', () => {
      setSetupScreenMode('settings');
      setAppScreen('onboarding');
      setStatus('Settings opened. Manage local model setup here.', 'neutral');
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
    if (dictationHotkeyInputEl) {
      dictationHotkeyInputEl.addEventListener('input', (event) => {
        pendingDictationHotkey = String(event?.currentTarget?.value || '').trim();
        isCapturingDictationHotkey = false;
        if (!pendingDictationHotkey) {
          setDictationHotkeyStatus(`Hotkey disabled. Default: ${defaultDictationHotkey}.`, 'neutral');
        } else {
          setDictationHotkeyStatus(`Pending hotkey: ${pendingDictationHotkey}. Click "Save Hotkey" to apply.`, 'neutral');
        }
        syncControls();
      });
    }
    if (dictationHotkeyPresetsEl) {
      dictationHotkeyPresetsEl.addEventListener('click', (event) => {
        const target = event.target instanceof Element
          ? event.target.closest('button[data-hotkey-preset]')
          : null;
        if (!target) return;

        pendingDictationHotkey = String(target.dataset.hotkeyPreset || '').trim();
        isCapturingDictationHotkey = false;
        if (dictationHotkeyInputEl) {
          dictationHotkeyInputEl.value = pendingDictationHotkey;
        }
        setDictationHotkeyStatus(`Preset selected: ${pendingDictationHotkey}. Click "Save Hotkey" to apply.`, 'neutral');
        syncControls();
      });
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
    if (focusedFieldInsertToggleEl) {
      focusedFieldInsertToggleEl.addEventListener('change', (event) => {
        const next = Boolean(event?.currentTarget?.checked);
        void saveFocusedFieldInsertSetting(next);
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
      void stopNativeDesktopDictation('button');
    });

    window.addEventListener('keydown', handleNativeHoldKeydown, true);
    window.addEventListener('keyup', handleNativeHoldKeyup, true);
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
        appendTranscriptChunk(chunk, { source: 'web' });
      } else {
        interimTranscript += chunk;
      }
    }

    transcriptInput.value = `${currentDraftText} ${interimTranscript}`.trim();
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

function getDictationTestState() {
  return {
    currentDraftText,
    dictationHistory: dictationHistory.map((entry) => ({ ...entry })),
    liveAudioLevel,
    liveAudioBars: [...liveAudioBars],
    waveformAudioState: dictationWaveformEl?.dataset?.audioState || 'idle',
    pendingNativeStartAfterStop,
    pendingNativeStartTrigger,
    nativeStopRequestInFlight,
    activeNativeSessionId,
    isDictating,
    isStartingDictation,
    dictationTriggerMode,
    dictationTriggerStatus,
    savedDictationHotkey,
    pendingDictationHotkey
  };
}

function resetDictationStateForTests() {
  dictationHistory = [];
  dictationHistorySeq = 0;
  isDictating = false;
  isStartingDictation = false;
  shouldKeepDictating = false;
  nativeStopRequestInFlight = false;
  pendingNativeStartAfterStop = false;
  pendingNativeStartTrigger = null;
  activeNativeSessionId = null;
  nativeSessionIdToIgnore = null;
  rejectNextNativeAppend = false;
  committedNativeSessionIds = new Set();
  startNativeDesktopDictationOverride = null;
  dictationTriggerMode = 'disabled';
  dictationTriggerStatus = 'Hotkey disabled.';
  dictationTriggerPermissionHint = '';
  focusedFieldInsertPermissionGranted = false;
  focusedFieldInsertPermissionStatus = 'Focused-field insertion is disabled.';
  savedDictationHotkey = null;
  pendingDictationHotkey = '';
  activeHotkeySpec = null;
  currentDeviceProfile = { os: 'macos', architecture: 'aarch64' };
  liveAudioLevel = 0;
  liveAudioBars = defaultLiveAudioBars();
  setDraftTranscriptText('');
  renderDictationHistory();
  resetDictationWaveform('idle');
  syncControls();
}

if (typeof globalThis !== 'undefined' && globalThis.__DICKTAINT_EXPOSE_TEST_API__) {
  globalThis.__DICKTAINT_TEST_API__ = {
    appendTranscriptChunk,
    runDictationHistoryAction,
    queueNativeStartAfterCurrentStop,
    maybeStartQueuedNativeDictation,
    setDraftTranscriptText,
    applyDictationHotkeyPayload,
    summarizeHotkeyPillStatus,
    handleNativeDictationStatePayload,
    handleNativeDictationAudioLevelPayload,
    getState: getDictationTestState,
    resetState: resetDictationStateForTests,
    setNativeFlags(next = {}) {
      if (typeof next.nativeStopRequestInFlight === 'boolean') {
        nativeStopRequestInFlight = next.nativeStopRequestInFlight;
      }
      if (typeof next.isDictating === 'boolean') isDictating = next.isDictating;
      if (typeof next.isStartingDictation === 'boolean') isStartingDictation = next.isStartingDictation;
      if (typeof next.pendingNativeStartAfterStop === 'boolean') {
        pendingNativeStartAfterStop = next.pendingNativeStartAfterStop;
      }
      if (typeof next.pendingNativeStartTrigger === 'string' || next.pendingNativeStartTrigger === null) {
        pendingNativeStartTrigger = next.pendingNativeStartTrigger;
      }
      syncControls();
    },
    setStartNativeDesktopDictationOverride(fn) {
      startNativeDesktopDictationOverride = typeof fn === 'function' ? fn : null;
    }
  };
}

setUiMode('loading');
setSetupScreenMode('onboarding');
setAppScreen('onboarding');
syncControls();
syncHotkeyPillForStatus(statusEl.textContent || '', 'neutral');
try {
  initDictation();
} catch (error) {
  const details = getErrorMessage(error);
  setUiMode('error');
  setStatus(`UI initialization failed: ${details}`, 'error');
}
initApp().catch((error) => {
  const details = getErrorMessage(error);
  setUiMode('error');
  setStatus(`Could not initialize setup: ${details}`, 'error');
});
