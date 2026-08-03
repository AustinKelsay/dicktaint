/**
 * Thin ESM entry for the dictation SPA.
 * Imports modules, initializes state, wires events, and optionally exposes test hooks.
 */
import { state } from './js/state.js';
import { dom } from './js/dom-elements.js';
import { DEFAULT_DICTATION_HOTKEY } from './js/constants.js';
import { isMacPlatform, isFocusedMacDesktopMode, getErrorMessage } from './js/platform.js';
import { defaultLiveAudioBars } from './js/waveform.js';
import {
  setUiMode,
  setStatus,
  setAppScreen,
  setSetupScreenMode,
  syncControls,
  syncHotkeyPillForStatus
} from './js/ui.js';
import { initDictation } from './js/events.js';
import { loadDictationOnboarding } from './js/onboarding/index.js';
import { createTestApi } from './js/test-api.js';

/**
 * Initializes runtime state and UI. Safe to call again in tests after replacing document mocks.
 */
export function bootstrapApp() {
  state.liveAudioBars = defaultLiveAudioBars();
  state.nativeDictationModelReady = !isFocusedMacDesktopMode();
  state.defaultDictationHotkey = isMacPlatform() ? 'Fn' : DEFAULT_DICTATION_HOTKEY;

  if (typeof globalThis !== 'undefined' && globalThis.__DICKTAINT_EXPOSE_TEST_API__) {
    globalThis.__DICKTAINT_TEST_API__ = createTestApi();
  }

  setUiMode('loading');
  setSetupScreenMode('onboarding');
  setAppScreen('onboarding');
  syncControls();
  syncHotkeyPillForStatus(dom.statusEl?.textContent || '', 'neutral');

  try {
    initDictation();
  } catch (error) {
    const details = getErrorMessage(error);
    setUiMode('error');
    setStatus(`UI initialization failed: ${details}`, 'error');
  }

  loadDictationOnboarding().catch((error) => {
    const details = getErrorMessage(error);
    setUiMode('error');
    setStatus(`Could not initialize setup: ${details}`, 'error');
  });
}

bootstrapApp();
