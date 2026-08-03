const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { describe, it, expect, beforeEach, afterEach, jest } = require('bun:test');

class MockElement {
  constructor(id = '', tagName = 'DIV') {
    this.id = id;
    this.tagName = String(tagName || 'DIV').toUpperCase();
    this.textContent = '';
    this.value = '';
    this.hidden = false;
    this.disabled = false;
    this.checked = false;
    this.placeholder = '';
    this.dataset = {};
    this.style = {
      setProperty(name, value) {
        this[name] = String(value);
      },
      getPropertyValue(name) {
        return this[name] || '';
      }
    };
    this.attributes = {};
    this.children = [];
    this.parentNode = null;
    this._listeners = new Map();
    this._innerHTML = '';
  }

  set innerHTML(value) {
    this._innerHTML = String(value || '');
    this.children = [];
  }

  get innerHTML() {
    return this._innerHTML;
  }

  get options() {
    return this.children;
  }

  addEventListener(type, handler) {
    if (!this._listeners.has(type)) this._listeners.set(type, []);
    this._listeners.get(type).push(handler);
  }

  dispatchEvent(event) {
    const payload = event || {};
    payload.target = payload.target || this;
    payload.currentTarget = this;
    payload.preventDefault = payload.preventDefault || (() => {});
    payload.stopPropagation = payload.stopPropagation || (() => {});
    const handlers = this._listeners.get(payload.type) || [];
    for (const handler of handlers) {
      handler(payload);
    }
  }

  click() {
    this.dispatchEvent({ type: 'click', target: this });
  }

  appendChild(child) {
    if (!child) return child;
    child.parentNode = this;
    this.children.push(child);
    return child;
  }

  remove() {
    if (!this.parentNode) return;
    this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
    this.parentNode = null;
  }

  focus() {}

  select() {}

  setAttribute(name, value) {
    this.attributes[name] = String(value);
  }

  getAttribute(name) {
    return this.attributes[name];
  }

  closest(selector) {
    if (selector === 'button[data-history-action][data-history-id]') {
      if (
        this.tagName === 'BUTTON'
        && this.dataset
        && this.dataset.historyAction
        && this.dataset.historyId
      ) {
        return this;
      }
      return null;
    }
    if (selector === 'button[data-hotkey-preset]') {
      if (
        this.tagName === 'BUTTON'
        && this.dataset
        && this.dataset.hotkeyPreset
      ) {
        return this;
      }
      return null;
    }
    return null;
  }
}

function createMockDom({ nativeDesktop = false, onboardingPayload = null, invokeHandler = null } = {}) {
  const ids = [
    'status',
    'onboardingScreen',
    'dictationScreen',
    'onboardingContinue',
    'openSettings',
    'setupModeChip',
    'setupTitle',
    'setupLead',
    'setupSteps',
    'startDictation',
    'stopDictation',
    'clearTranscript',
    'dictationWaveform',
    'dictationWaveformLevel',
    'dictationWaveBar0',
    'dictationWaveBar1',
    'dictationWaveBar2',
    'dictationWaveBar3',
    'dictationWaveBar4',
    'dictationWaveBar5',
    'dictationWaveBar6',
    'dictationWaveBar7',
    'dictationWaveBar8',
    'dictationWaveBar9',
    'dictationWaveBar10',
    'dictationWaveBar11',
    'transcriptInput',
    'dictationHistorySection',
    'dictationHistoryList',
    'dictationHistoryEmpty',
    'clearDictationHistory',
    'dictationModelCard',
    'dictationModelSelect',
    'installDictationModel',
    'deleteDictationModel',
    'openWhisperSetup',
    'retryWhisperCheck',
    'whisperCliHealth',
    'dictationModelHealth',
    'dictationModelStatus',
    'dictationModelBusy',
    'dictationDeviceProfile',
    'dictationModelMeta',
    'dictationHotkeyCard',
    'dictationHotkeyInput',
    'recordDictationHotkey',
    'saveDictationHotkey',
    'resetDictationHotkey',
    'clearDictationHotkey',
    'dictationHotkeyStatus',
    'dictationHotkeyPresets',
    'focusedFieldInsertCard',
    'focusedFieldInsertToggle',
    'focusedFieldInsertStatus',
    'backgroundUiCard',
    'menuBarModeSelect',
    'closeActionSelect',
    'pillVisibilityModeSelect',
    'backgroundUiStatus',
    'dictationPermissionsCard',
    'dictationPermissionSummary',
    'dictationPermissionList',
    'quickDictationFab'
  ];

  const elements = new Map();
  for (const id of ids) {
    const tagName = id.toLowerCase().includes('button') || id.startsWith('clear') ? 'BUTTON' : 'DIV';
    elements.set(id, new MockElement(id, tagName));
  }
  elements.get('transcriptInput').tagName = 'TEXTAREA';
  elements.get('dictationModelSelect').tagName = 'SELECT';
  elements.get('dictationHotkeyInput').tagName = 'INPUT';
  elements.get('focusedFieldInsertToggle').tagName = 'INPUT';
  elements.get('menuBarModeSelect').tagName = 'SELECT';
  elements.get('closeActionSelect').tagName = 'SELECT';
  elements.get('pillVisibilityModeSelect').tagName = 'SELECT';
  elements.get('dictationPermissionList').tagName = 'UL';
  elements.get('status').textContent = 'Loading...';

  const appShell = new MockElement('appShell', 'DIV');
  const body = {
    dataset: {},
    style: {}
  };

  const documentListeners = new Map();
  const clipboardCalls = [];
  const invokeCalls = [];
  const emitCalls = [];
  const backgroundPreferences = {
    pill_visibility_mode: onboardingPayload?.pill_visibility_mode || 'active-only',
    menu_bar_mode: onboardingPayload?.menu_bar_mode || 'always',
    close_action: onboardingPayload?.close_action || 'hide-to-tray'
  };
  if (backgroundPreferences.menu_bar_mode === 'off') {
    backgroundPreferences.close_action = 'quit';
  }

  const defaultModels = [
    {
      id: 'base-en',
      display_name: 'Whisper Base (English)',
      approx_size_gb: 0.15,
      speed_note: 'Fast',
      quality_note: 'Balanced',
      installed: true,
      recommended: true,
      likely_runnable: true
    },
    {
      id: 'small-en',
      display_name: 'Whisper Small (English)',
      approx_size_gb: 0.5,
      speed_note: 'Balanced',
      quality_note: 'Higher quality',
      installed: false,
      recommended: false,
      likely_runnable: true
    }
  ];

  const onboardingState = {
    onboarding_required: false,
    selected_model_id: 'base-en',
    selected_model_path: '/tmp/ggml-base.en.bin',
    selected_model_exists: true,
    available_input_devices: [],
    preferred_input_device: null,
    dictation_trigger: 'Fn',
    default_dictation_trigger: 'Fn',
    dictation_trigger_mode: 'global-hold',
    dictation_trigger_status: 'Hold Fn anywhere to dictate, then release to transcribe.',
    dictation_trigger_permission_hint: null,
    focused_field_insert_enabled: false,
    focused_field_insert_permission_granted: true,
    focused_field_insert_permission_status: 'Accessibility permission granted. Finished transcripts can be pasted into the focused field of other apps.',
    whisper_cli_available: true,
    whisper_cli_path: '/usr/local/bin/whisper-cli',
    models_dir: '/tmp/models',
    device: {
      total_memory_gb: 16,
      logical_cpu_cores: 8,
      architecture: 'aarch64',
      os: 'macos'
    },
    models: defaultModels.map((model) => ({ ...model })),
    ...(onboardingPayload || {})
  };
  if (Array.isArray(onboardingPayload?.models)) {
    onboardingState.models = onboardingPayload.models.map((model) => ({ ...model }));
  }

  global.Element = MockElement;
  global.navigator = {
    platform: 'MacIntel',
    userAgent: 'MockDesktop',
    clipboard: {
      writeText: async (value) => {
        clipboardCalls.push(value);
      }
    }
  };

  global.document = {
    body,
    getElementById(id) {
      return elements.get(id) || null;
    },
    querySelector(selector) {
      if (selector === '.app-shell') return appShell;
      return null;
    },
    createElement(tagName) {
      return new MockElement('', tagName);
    },
    addEventListener(type, handler) {
      if (!documentListeners.has(type)) documentListeners.set(type, []);
      documentListeners.get(type).push(handler);
    },
    execCommand(command) {
      return command === 'copy';
    },
    hasFocus() {
      return false;
    }
  };

  const windowListeners = new Map();
  global.window = {
    confirm: () => true,
    __TAURI__: nativeDesktop ? {
      core: {
        invoke: async (command, args = {}) => {
          invokeCalls.push({ command, args });
          if (typeof invokeHandler === 'function') {
            const handled = await invokeHandler(command, args, {
              backgroundPreferences,
              invokeCalls,
              onboardingState
            });
            if (handled !== undefined) return handled;
          }
          if (command === 'get_dictation_onboarding') {
            return {
              ...onboardingState,
              ...backgroundPreferences,
              models: onboardingState.models.map((model) => ({ ...model }))
            };
          }
          if (command === 'set_dictation_trigger') {
            const trigger = String(args.trigger || '').trim();
            onboardingState.dictation_trigger = trigger;
            onboardingState.dictation_trigger_mode = trigger === 'Fn' ? 'global-hold' : 'global-toggle';
            onboardingState.dictation_trigger_status = trigger
              ? `Hotkey set to ${trigger}.`
              : 'Hotkey disabled.';
            return {
              trigger,
              default_trigger: onboardingState.default_dictation_trigger,
              trigger_mode: onboardingState.dictation_trigger_mode,
              trigger_status: onboardingState.dictation_trigger_status,
              trigger_permission_hint: onboardingState.dictation_trigger_permission_hint
            };
          }
          if (command === 'clear_dictation_trigger') {
            onboardingState.dictation_trigger = '';
            onboardingState.dictation_trigger_mode = 'disabled';
            onboardingState.dictation_trigger_status = 'Hotkey disabled.';
            return {
              trigger: '',
              default_trigger: onboardingState.default_dictation_trigger,
              trigger_mode: 'disabled',
              trigger_status: 'Hotkey disabled.',
              trigger_permission_hint: null
            };
          }
          if (command === 'set_focused_field_insert_enabled') {
            const enabled = Boolean(args.enabled);
            onboardingState.focused_field_insert_enabled = enabled;
            onboardingState.focused_field_insert_permission_granted = true;
            onboardingState.focused_field_insert_permission_status = enabled
              ? 'Accessibility permission granted. Finished transcripts can be pasted into the focused field of other apps.'
              : 'Focused-field insertion is disabled.';
            return {
              focused_field_insert_enabled: enabled,
              focused_field_insert_permission_granted: true,
              focused_field_insert_permission_status: onboardingState.focused_field_insert_permission_status
            };
          }
          if (command === 'insert_text_into_focused_field') {
            return null;
          }
          if (command === 'install_dictation_model') {
            const modelId = String(args.model || '').trim();
            const model = onboardingState.models.find((item) => item.id === modelId);
            if (model) model.installed = true;
            onboardingState.selected_model_id = modelId;
            onboardingState.selected_model_path = `/tmp/ggml-${modelId}.bin`;
            onboardingState.selected_model_exists = true;
            return null;
          }
          if (command === 'delete_dictation_model') {
            const modelId = String(args.model || '').trim();
            const model = onboardingState.models.find((item) => item.id === modelId);
            if (model) model.installed = false;
            if (onboardingState.selected_model_id === modelId) {
              const fallback = onboardingState.models.find((item) => item.installed);
              if (fallback) {
                onboardingState.selected_model_id = fallback.id;
                onboardingState.selected_model_path = `/tmp/ggml-${fallback.id}.bin`;
                onboardingState.selected_model_exists = true;
              } else {
                onboardingState.selected_model_exists = false;
                onboardingState.selected_model_path = '';
              }
            }
            return null;
          }
          if (command === 'set_pill_visibility_mode') {
            backgroundPreferences.pill_visibility_mode = args.mode || 'active-only';
            return { ...backgroundPreferences };
          }
          if (command === 'set_menu_bar_mode') {
            backgroundPreferences.menu_bar_mode = args.mode || 'always';
            if (backgroundPreferences.menu_bar_mode === 'off') {
              backgroundPreferences.close_action = 'quit';
            }
            return { ...backgroundPreferences };
          }
          if (command === 'set_close_action') {
            backgroundPreferences.close_action = backgroundPreferences.menu_bar_mode === 'off'
              ? 'quit'
              : (args.action || 'hide-to-tray');
            return { ...backgroundPreferences };
          }
          throw new Error(`Unhandled Tauri command in test: ${command}`);
        }
      },
      event: {
        listen: async () => () => {},
        emit: async (event, payload) => {
          emitCalls.push({ event, payload });
        }
      }
    } : null,
    navigator: global.navigator,
    addEventListener(type, handler) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(handler);
    },
    open() {}
  };

  return { clipboardCalls, invokeCalls, emitCalls, onboardingState };
}

const appModulePath = path.join(__dirname, '../public/app.js');
let appModulePromise = null;

async function loadAppWithTestApi() {
  global.__DICKTAINT_EXPOSE_TEST_API__ = true;
  delete global.__DICKTAINT_TEST_API__;
  if (!appModulePromise) {
    appModulePromise = import(pathToFileURL(appModulePath).href);
  }
  const appModule = await appModulePromise;
  appModule.bootstrapApp();
  await Promise.resolve();
  await Promise.resolve();
  if (!global.__DICKTAINT_TEST_API__) {
    const { createTestApi } = await import(pathToFileURL(path.join(__dirname, '../public/js/test-api.js')).href);
    global.__DICKTAINT_TEST_API__ = createTestApi();
  }
  return global.__DICKTAINT_TEST_API__;
}

describe('dictation frontend history + chaining', () => {
  let api;
  let clipboardCalls;

  beforeEach(async () => {
    ({ clipboardCalls } = createMockDom());
    api = await loadAppWithTestApi();
    api.resetState();
  });

  afterEach(() => {
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('keeps a rolling history of the most recent 10 dictations', () => {
    for (let i = 1; i <= 12; i += 1) {
      api.appendTranscriptChunk(`chunk ${i}`, {
        source: 'native',
        nativeSessionId: `session-${i}`
      });
    }

    const state = api.getState();
    expect(state.dictationHistory).toHaveLength(10);
    expect(state.dictationHistory[0].text).toBe('chunk 12');
    expect(state.dictationHistory[9].text).toBe('chunk 3');
  });

  it('does not commit the same native session transcript twice', () => {
    const first = api.appendTranscriptChunk('hello world', {
      source: 'native',
      nativeSessionId: 'session-1'
    });
    const second = api.appendTranscriptChunk('hello world', {
      source: 'native-event',
      nativeSessionId: 'session-1'
    });

    const state = api.getState();
    expect(first).toBe(true);
    expect(second).toBe(false);
    expect(state.currentDraftText).toBe('hello world');
    expect(state.dictationHistory).toHaveLength(1);
  });

  it('queues the next dictation start while stop/transcribe is in flight', async () => {
    const starts = [];
    api.setStartNativeDesktopDictationOverride(async (trigger) => {
      starts.push(trigger);
    });

    api.setNativeFlags({ nativeStopRequestInFlight: true });
    api.queueNativeStartAfterCurrentStop('hotkey');

    await api.maybeStartQueuedNativeDictation();
    expect(starts).toHaveLength(0);

    api.setNativeFlags({ nativeStopRequestInFlight: false });
    await api.maybeStartQueuedNativeDictation();
    expect(starts).toEqual(['hotkey']);

    const state = api.getState();
    expect(state.pendingNativeStartAfterStop).toBe(false);
    expect(state.pendingNativeStartTrigger).toBeNull();
  });

  it('supports reinsert action from recent dictation history', async () => {
    api.appendTranscriptChunk('first entry', {
      source: 'native',
      nativeSessionId: 'session-1'
    });
    api.appendTranscriptChunk('second entry', {
      source: 'native',
      nativeSessionId: 'session-2'
    });

    const latest = api.getState().dictationHistory[0];
    api.setDraftTranscriptText('');

    const ok = await api.runDictationHistoryAction('reinsert', latest.id);
    expect(ok).toBe(true);
    expect(api.getState().currentDraftText).toBe('second entry');
  });

  it('supports copy action from recent dictation history', async () => {
    api.appendTranscriptChunk('copy this chunk', {
      source: 'native',
      nativeSessionId: 'session-copy'
    });
    const latest = api.getState().dictationHistory[0];

    const ok = await api.runDictationHistoryAction('copy', latest.id);
    expect(ok).toBe(true);
    expect(clipboardCalls).toEqual(['copy this chunk']);
  });

  it('smoke: send + immediate next dictation keeps history and chains start', async () => {
    api.appendTranscriptChunk('message one', {
      source: 'native',
      nativeSessionId: 'session-a'
    });
    api.setDraftTranscriptText('');

    const starts = [];
    api.setStartNativeDesktopDictationOverride(async (trigger) => {
      starts.push(trigger);
    });

    api.setNativeFlags({ nativeStopRequestInFlight: true });
    api.queueNativeStartAfterCurrentStop('hotkey');
    api.setNativeFlags({ nativeStopRequestInFlight: false });
    await api.maybeStartQueuedNativeDictation();

    const state = api.getState();
    expect(starts).toEqual(['hotkey']);
    expect(state.currentDraftText).toBe('');
    expect(state.dictationHistory).toHaveLength(1);
    expect(state.dictationHistory[0].text).toBe('message one');
  });
});

describe('dictation frontend hotkey polish', () => {
  let api;

  beforeEach(async () => {
    createMockDom({ nativeDesktop: true });
    api = await loadAppWithTestApi();
    api.resetState();
  });

  afterEach(() => {
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('tracks focused-window fn fallback from backend payloads', () => {
    api.applyDictationHotkeyPayload({
      trigger: 'Fn',
      default_trigger: 'Fn',
      trigger_mode: 'focused-window-hold',
      trigger_status: 'Hold Fn to dictate while dicktaint is focused. Grant Input Monitoring for global hold-to-talk.',
      trigger_permission_hint: 'Allow Input Monitoring for dicktaint.'
    });

    const state = api.getState();
    expect(state.dictationTriggerMode).toBe('focused-window-hold');
    expect(state.savedDictationHotkey).toBe('Fn');
    expect(document.getElementById('dictationHotkeyStatus').textContent).toContain('Current hotkey: Fn');
    expect(document.getElementById('dictationHotkeyStatus').textContent).toContain('focused');
  });

  it('renders pill messaging from the active hotkey mode', () => {
    api.applyDictationHotkeyPayload({
      trigger: 'CmdOrCtrl+Shift+D',
      default_trigger: 'Fn',
      trigger_mode: 'global-toggle',
      trigger_status: 'Press CmdOrCtrl+Shift+D anywhere to start or stop dictation.'
    });

    expect(api.summarizeHotkeyPillStatus('Listening...', 'live')).toBe('Listening - press CmdOrCtrl+Shift+D again');

    api.applyDictationHotkeyPayload({
      trigger: 'Fn',
      default_trigger: 'Fn',
      trigger_mode: 'global-hold',
      trigger_status: 'Hold Fn anywhere to dictate, then release to transcribe.'
    });

    expect(api.summarizeHotkeyPillStatus('Listening...', 'live')).toBe('Listening - release Fn / Globe');
  });

  it('keeps a newer live session active when an older transcript finishes later', () => {
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 1
    });
    api.handleNativeDictationStatePayload({
      state: 'processing',
      session_id: 1
    });
    // startNativeDesktopDictation clears the prior session id before the next listen event.
    api.setNativeFlags({ activeNativeSessionId: null });
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 2
    });
    api.handleNativeDictationStatePayload({
      state: 'idle',
      session_id: 1,
      transcript: 'first session'
    });

    const state = api.getState();
    expect(state.isDictating).toBe(true);
    expect(state.activeNativeSessionId).toBe('2');
    expect(state.currentDraftText).toBe('first session');
  });

  it('ignores late listening events from a superseded session', () => {
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 2
    });
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 1
    });

    const state = api.getState();
    expect(state.isDictating).toBe(true);
    expect(state.activeNativeSessionId).toBe('2');
  });

  it('renders live mic levels from native audio payloads and ignores stale sessions', () => {
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 4
    });

    api.handleNativeDictationAudioLevelPayload({
      session_id: 4,
      level: 0.52,
      bars: [0.1, 0.18, 0.26, 0.34, 0.42, 0.5, 0.58, 0.66, 0.74, 0.82, 0.9, 1]
    });
    api.handleNativeDictationAudioLevelPayload({
      session_id: 3,
      level: 1,
      bars: Array.from({ length: 12 }, () => 1)
    });

    const state = api.getState();
    expect(state.liveAudioLevel).toBe(0.52);
    expect(state.waveformAudioState).toBe('ready');
    expect(state.liveAudioBars[11]).toBe(1);
    expect(document.getElementById('dictationWaveformLevel').textContent).toBe('Mic level: good');
    expect(document.getElementById('dictationWaveBar11').style.getPropertyValue('--bar-level')).toBe('1.000');
  });
});

describe('dictation frontend background UI settings', () => {
  let api;
  let invokeCalls;

  async function flushUi() {
    await Promise.resolve();
    await Promise.resolve();
  }

  beforeEach(async () => {
    ({ invokeCalls } = createMockDom({
      nativeDesktop: true,
      onboardingPayload: {
        onboarding_required: false,
        selected_model_id: 'base-en',
        selected_model_path: '/tmp/ggml-base.en.bin',
        selected_model_exists: true,
        available_input_devices: [],
        preferred_input_device: null,
        dictation_trigger: 'Fn',
        default_dictation_trigger: 'Fn',
        dictation_trigger_mode: 'global-hold',
        dictation_trigger_status: 'Hold Fn anywhere to dictate, then release to transcribe.',
        dictation_trigger_permission_hint: null,
        pill_visibility_mode: 'always',
        menu_bar_mode: 'background-only',
        close_action: 'hide-to-tray',
        focused_field_insert_enabled: false,
        focused_field_insert_permission_granted: true,
        focused_field_insert_permission_status: 'Accessibility permission granted.',
        whisper_cli_available: true,
        whisper_cli_path: '/usr/local/bin/whisper-cli',
        models_dir: '/tmp/models',
        device: {
          total_memory_gb: 16,
          logical_cpu_cores: 8,
          architecture: 'aarch64',
          os: 'macos'
        },
        models: [
          {
            id: 'base-en',
            display_name: 'Whisper Base (English)',
            approx_size_gb: 0.15,
            speed_note: 'Fast',
            quality_note: 'Balanced',
            installed: true,
            recommended: true,
            likely_runnable: true
          }
        ]
      }
    }));
    api = await loadAppWithTestApi();
    await flushUi();
  });

  afterEach(() => {
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('hydrates background UI preferences from onboarding payload', () => {
    const state = api.getState();
    expect(state.menuBarMode).toBe('background-only');
    expect(state.closeAction).toBe('hide-to-tray');
    expect(state.pillVisibilityMode).toBe('always');
    expect(document.getElementById('menuBarModeSelect').value).toBe('background-only');
    expect(document.getElementById('closeActionSelect').value).toBe('hide-to-tray');
    expect(document.getElementById('pillVisibilityModeSelect').value).toBe('always');
  });

  it('saves pill visibility mode changes through the Tauri command', async () => {
    const select = document.getElementById('pillVisibilityModeSelect');
    select.value = 'off';
    select.dispatchEvent({ type: 'change', currentTarget: select });
    await flushUi();

    expect(invokeCalls.at(-1)).toEqual({
      command: 'set_pill_visibility_mode',
      args: { mode: 'off' }
    });
    expect(api.getState().pillVisibilityMode).toBe('off');
  });

  it('forces close action to quit when the menu bar mode is turned off', async () => {
    const select = document.getElementById('menuBarModeSelect');
    select.value = 'off';
    select.dispatchEvent({ type: 'change', currentTarget: select });
    await flushUi();

    const state = api.getState();
    expect(invokeCalls.at(-1)).toEqual({
      command: 'set_menu_bar_mode',
      args: { mode: 'off' }
    });
    expect(state.menuBarMode).toBe('off');
    expect(state.closeAction).toBe('quit');
    expect(document.getElementById('closeActionSelect').value).toBe('quit');
    expect(document.getElementById('closeActionSelect').disabled).toBe(true);
  });

  it('keeps dictation status rendering stable while background UI preferences change', () => {
    // Clear any leftover session id from prior describes (module state is shared).
    api.setNativeFlags({ activeNativeSessionId: null, isDictating: false });
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 42
    });
    api.applyBackgroundUiPreferencesPayload({
      pill_visibility_mode: 'off',
      menu_bar_mode: 'always',
      close_action: 'hide-to-tray'
    });

    expect(api.getState().isDictating).toBe(true);
    expect(document.getElementById('status').textContent).toBe('Listening… click Stop to transcribe.');
  });
});

describe('dictation frontend hotkey save/clear', () => {
  let api;
  let invokeCalls;

  async function flushUi() {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  }

  beforeEach(async () => {
    ({ invokeCalls } = createMockDom({ nativeDesktop: true }));
    api = await loadAppWithTestApi();
    await flushUi();
    api.resetState();
  });

  afterEach(() => {
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('saves a hotkey combo through set_dictation_trigger and updates state', async () => {
    await api.saveDictationHotkey('CmdOrCtrl+Shift+D');
    await flushUi();

    const saveCall = invokeCalls.find((call) => call.command === 'set_dictation_trigger');
    expect(saveCall).toEqual({
      command: 'set_dictation_trigger',
      args: { trigger: 'CmdOrCtrl+Shift+D' }
    });

    const state = api.getState();
    expect(state.savedDictationHotkey).toBe('CmdOrCtrl+Shift+D');
    expect(state.dictationTriggerMode).toBe('global-toggle');
    expect(document.getElementById('dictationHotkeyStatus').textContent).toContain('Current hotkey: CmdOrCtrl+Shift+D');
  });

  it('clears the saved hotkey through clear_dictation_trigger', async () => {
    await api.saveDictationHotkey('Fn');
    await flushUi();
    await api.clearDictationHotkey();
    await flushUi();

    const clearCall = invokeCalls.find((call) => call.command === 'clear_dictation_trigger');
    expect(clearCall).toEqual({
      command: 'clear_dictation_trigger',
      args: {}
    });

    const state = api.getState();
    expect(state.savedDictationHotkey).toBeNull();
    expect(state.dictationTriggerMode).toBe('disabled');
    expect(document.getElementById('dictationHotkeyStatus').textContent).toContain('Hotkey disabled');
  });

  it('rejects invalid hotkey combos without invoking Tauri', async () => {
    const before = invokeCalls.length;
    await api.saveDictationHotkey('NotARealKeyCombo!!!');
    await flushUi();

    expect(invokeCalls.slice(before).some((call) => call.command === 'set_dictation_trigger')).toBe(false);
    expect(document.getElementById('dictationHotkeyStatus').textContent.length).toBeGreaterThan(0);
  });
});

describe('dictation frontend focused-field insert', () => {
  let api;
  let invokeCalls;

  async function flushUi() {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  }

  beforeEach(async () => {
    ({ invokeCalls } = createMockDom({ nativeDesktop: true }));
    api = await loadAppWithTestApi();
    await flushUi();
    api.resetState();
  });

  afterEach(() => {
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('saves the focused-field insert toggle through Tauri', async () => {
    await api.saveFocusedFieldInsertSetting(true);
    await flushUi();

    expect(invokeCalls.at(-1)).toEqual({
      command: 'set_focused_field_insert_enabled',
      args: { enabled: true }
    });

    const state = api.getState();
    expect(state.focusedFieldInsertEnabled).toBe(true);
    expect(state.focusedFieldInsertPermissionGranted).toBe(true);
    expect(document.getElementById('focusedFieldInsertToggle').checked).toBe(true);
  });

  it('inserts finished transcripts into the focused field when enabled', async () => {
    api.applyFocusedFieldInsertPayload({
      focused_field_insert_enabled: true,
      focused_field_insert_permission_granted: true,
      focused_field_insert_permission_status: 'Accessibility permission granted.'
    });

    api.appendTranscriptChunk('paste me elsewhere', {
      source: 'native',
      nativeSessionId: 'session-insert-1'
    });
    await flushUi();

    const insertCall = invokeCalls.find((call) => call.command === 'insert_text_into_focused_field');
    expect(insertCall).toEqual({
      command: 'insert_text_into_focused_field',
      args: { text: 'paste me elsewhere' }
    });
  });

  it('skips focused-field insert when the setting is disabled', async () => {
    api.applyFocusedFieldInsertPayload({
      focused_field_insert_enabled: false,
      focused_field_insert_permission_granted: true,
      focused_field_insert_permission_status: 'Focused-field insertion is disabled.'
    });

    const before = invokeCalls.length;
    api.appendTranscriptChunk('stay local only', {
      source: 'native',
      nativeSessionId: 'session-insert-skip'
    });
    await flushUi();

    expect(invokeCalls.slice(before).some((call) => call.command === 'insert_text_into_focused_field')).toBe(false);
  });
});

describe('dictation frontend onboarding model install/delete', () => {
  let api;
  let invokeCalls;

  async function flushUi() {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  }

  beforeEach(async () => {
    ({ invokeCalls } = createMockDom({
      nativeDesktop: true,
      onboardingPayload: {
        selected_model_id: 'base-en',
        selected_model_exists: true,
        whisper_cli_available: true,
        models: [
          {
            id: 'base-en',
            display_name: 'Whisper Base (English)',
            approx_size_gb: 0.15,
            speed_note: 'Fast',
            quality_note: 'Balanced',
            installed: true,
            recommended: true,
            likely_runnable: true
          },
          {
            id: 'small-en',
            display_name: 'Whisper Small (English)',
            approx_size_gb: 0.5,
            speed_note: 'Balanced',
            quality_note: 'Higher quality',
            installed: false,
            recommended: false,
            likely_runnable: true
          }
        ]
      }
    }));
    api = await loadAppWithTestApi();
    await flushUi();
  });

  afterEach(() => {
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('installs the selected model via install_dictation_model', async () => {
    document.getElementById('dictationModelSelect').value = 'small-en';
    await api.installSelectedDictationModel();
    await flushUi();

    const installCall = invokeCalls.find((call) => call.command === 'install_dictation_model');
    expect(installCall).toEqual({
      command: 'install_dictation_model',
      args: { model: 'small-en' }
    });
    expect(invokeCalls.some((call) => call.command === 'get_dictation_onboarding')).toBe(true);
    expect(api.getState().nativeDictationModelReady).toBe(true);
  });

  it('deletes the selected installed model via delete_dictation_model', async () => {
    document.getElementById('dictationModelSelect').value = 'base-en';
    global.window.confirm = () => true;

    await api.deleteSelectedDictationModel();
    await flushUi();

    const deleteCall = invokeCalls.find((call) => call.command === 'delete_dictation_model');
    expect(deleteCall).toEqual({
      command: 'delete_dictation_model',
      args: { model: 'base-en' }
    });
  });

  it('does not delete when confirmation is canceled', async () => {
    document.getElementById('dictationModelSelect').value = 'base-en';
    global.window.confirm = () => false;
    const before = invokeCalls.length;

    await api.deleteSelectedDictationModel();
    await flushUi();

    expect(invokeCalls.slice(before).some((call) => call.command === 'delete_dictation_model')).toBe(false);
  });
});

describe('dictation frontend web speech error/restart', () => {
  let api;

  beforeEach(async () => {
    createMockDom();
    api = await loadAppWithTestApi();
    api.resetState();
  });

  afterEach(() => {
    jest.useRealTimers();
    api.clearRestartTimer();
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('classifies fatal vs restartable speech errors', () => {
    expect(api.isFatalSpeechError('not-allowed')).toBe(true);
    expect(api.isFatalSpeechError('audio-capture')).toBe(true);
    expect(api.isFatalSpeechError('network')).toBe(true);
    expect(api.isFatalSpeechError('language-not-supported')).toBe(true);
    expect(api.isFatalSpeechError('no-speech')).toBe(false);
    expect(api.isFatalSpeechError('aborted')).toBe(false);
    expect(api.describeSpeechError('no-speech')).toContain('no speech');
  });

  it('schedules a recognition restart when dictation should continue', () => {
    jest.useFakeTimers();
    const starts = [];
    api.setRecognition({
      start() {
        starts.push('start');
      },
      stop() {}
    });
    api.setNativeFlags({
      shouldKeepDictating: true,
      isDictating: false,
      isStartingDictation: false
    });

    api.scheduleRecognitionRestart();
    expect(api.getState().hasRestartTimer).toBe(true);
    expect(starts).toHaveLength(0);

    jest.advanceTimersByTime(250);
    expect(starts).toEqual(['start']);
    expect(document.body.dataset.mode).toBe('loading');
    expect(document.getElementById('status').textContent).toBe('Reconnecting dictation...');
  });

  it('does not restart when a fatal-style keep flag is cleared', () => {
    jest.useFakeTimers();
    const starts = [];
    api.setRecognition({
      start() {
        starts.push('start');
      },
      stop() {}
    });
    api.setNativeFlags({
      shouldKeepDictating: false,
      isDictating: false,
      isStartingDictation: false
    });

    api.scheduleRecognitionRestart();
    jest.advanceTimersByTime(250);
    expect(starts).toHaveLength(0);
  });
});

describe('dictation frontend overlay/pill payloads', () => {
  let api;
  let emitCalls;

  beforeEach(async () => {
    ({ emitCalls } = createMockDom({ nativeDesktop: true }));
    api = await loadAppWithTestApi();
    api.resetState();
  });

  afterEach(() => {
    delete global.__DICKTAINT_TEST_API__;
    delete global.__DICKTAINT_EXPOSE_TEST_API__;
    delete global.window;
    delete global.document;
    delete global.navigator;
    delete global.Element;
  });

  it('emits pill overlay status payloads through Tauri events', async () => {
    await api.setHotkeyPill('Listening - release Fn / Globe', 'live', true);
    await Promise.resolve();

    expect(emitCalls.at(-1)).toEqual({
      event: 'dicktaint://pill-status',
      payload: {
        message: 'Listening - release Fn / Globe',
        state: 'live',
        visible: true
      }
    });
  });

  it('handles processing then idle transcript payloads and resets session', () => {
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 7
    });
    api.handleNativeDictationStatePayload({
      state: 'processing',
      session_id: 7
    });
    expect(api.getState().isDictating).toBe(true);
    expect(document.getElementById('status').textContent).toContain('Transcribing');

    api.handleNativeDictationStatePayload({
      state: 'idle',
      session_id: 7,
      transcript: 'finished phrase'
    });

    const state = api.getState();
    expect(state.isDictating).toBe(false);
    expect(state.activeNativeSessionId).toBeNull();
    expect(state.currentDraftText).toBe('finished phrase');
    expect(state.dictationHistory[0].text).toBe('finished phrase');
  });

  it('surfaces native dictation error payloads and clears the active session', () => {
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 9
    });
    api.handleNativeDictationStatePayload({
      state: 'error',
      session_id: 9,
      error: 'mic unavailable'
    });

    const state = api.getState();
    expect(state.isDictating).toBe(false);
    expect(state.activeNativeSessionId).toBeNull();
    expect(document.body.dataset.mode).toBe('error');
    expect(document.getElementById('status').textContent).toContain('mic unavailable');
  });

  it('ignores audio-level payloads for stale sessions and applies matching ones', () => {
    api.handleNativeDictationStatePayload({
      state: 'listening',
      session_id: 11
    });

    api.handleNativeDictationAudioLevelPayload({
      session_id: 99,
      level: 0.95,
      bars: Array.from({ length: 12 }, () => 0.95)
    });
    expect(api.getState().liveAudioLevel).toBe(0);

    api.handleNativeDictationAudioLevelPayload({
      session_id: 11,
      level: 0.2,
      bars: [0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6]
    });

    const state = api.getState();
    expect(state.liveAudioLevel).toBe(0.2);
    expect(state.waveformAudioState).toBe('ready');
    expect(document.getElementById('dictationWaveBar0').style.getPropertyValue('--bar-level')).toBe('0.050');
  });
});
