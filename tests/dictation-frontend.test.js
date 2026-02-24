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
    this.style = {};
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
    return null;
  }
}

function createMockDom() {
  const ids = [
    'status',
    'onboardingScreen',
    'dictationScreen',
    'onboardingContinue',
    'openSettings',
    'backToDictation',
    'setupModeChip',
    'setupTitle',
    'setupLead',
    'setupSteps',
    'startDictation',
    'stopDictation',
    'clearTranscript',
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
    'focusedFieldInsertCard',
    'focusedFieldInsertToggle',
    'focusedFieldInsertStatus',
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
  elements.get('status').textContent = 'Loading...';

  const appShell = new MockElement('appShell', 'DIV');
  const body = {
    dataset: {},
    style: {}
  };

  const documentListeners = new Map();
  const clipboardCalls = [];

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
    __TAURI__: null,
    navigator: global.navigator,
    addEventListener(type, handler) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(handler);
    },
    open() {}
  };

  return { clipboardCalls };
}

function loadAppWithTestApi() {
  global.__DICKTAINT_EXPOSE_TEST_API__ = true;
  delete global.__DICKTAINT_TEST_API__;
  delete require.cache[require.resolve('../public/app.js')];
  require('../public/app.js');
  return global.__DICKTAINT_TEST_API__;
}

describe('dictation frontend history + chaining', () => {
  let api;
  let clipboardCalls;

  beforeEach(() => {
    ({ clipboardCalls } = createMockDom());
    api = loadAppWithTestApi();
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
