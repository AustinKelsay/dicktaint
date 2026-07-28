const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { describe, it, expect, beforeEach, afterEach } = require('bun:test');

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
  const backgroundPreferences = {
    pill_visibility_mode: onboardingPayload?.pill_visibility_mode || 'active-only',
    menu_bar_mode: onboardingPayload?.menu_bar_mode || 'always',
    close_action: onboardingPayload?.close_action || 'hide-to-tray'
  };
  if (backgroundPreferences.menu_bar_mode === 'off') {
    backgroundPreferences.close_action = 'quit';
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
    }
  };

  const windowListeners = new Map();
  global.window = {
    __TAURI__: nativeDesktop ? {
      core: {
        invoke: async (command, args = {}) => {
          invokeCalls.push({ command, args });
          if (typeof invokeHandler === 'function') {
            const handled = await invokeHandler(command, args, { backgroundPreferences, invokeCalls });
            if (handled !== undefined) return handled;
          }
          if (command === 'get_dictation_onboarding') {
            if (onboardingPayload) {
              return {
                ...onboardingPayload,
                ...backgroundPreferences
              };
            }
            return {
              onboarding_required: false,
              selected_model_id: 'base-en',
              selected_model_path: '/tmp/ggml-base.en.bin',
              selected_model_exists: true,
              dictation_trigger: 'Fn',
              default_dictation_trigger: 'Fn',
              dictation_trigger_mode: 'global-hold',
              dictation_trigger_status: 'Hold Fn anywhere to dictate, then release to transcribe.',
              dictation_trigger_permission_hint: null,
              ...backgroundPreferences,
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
            };
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
        emit: async () => {}
      }
    } : null,
    navigator: global.navigator,
    addEventListener(type, handler) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(handler);
    },
    open() {}
  };

  return { clipboardCalls, invokeCalls };
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
