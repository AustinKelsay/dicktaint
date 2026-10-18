/** Owns screen choice. Readiness chooses the initial route; quiet checks never navigate. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { isNativeSetupReady } from './readiness.js';
import { isNativeDesktopMode } from './platform.js';

export function setAppScreen(screen, { focus = true } = {}) {
  const next = screen === 'dictation' ? 'dictation' : 'onboarding';
  if (dom.onboardingScreen) dom.onboardingScreen.hidden = next !== 'onboarding';
  if (dom.dictationScreen) dom.dictationScreen.hidden = next !== 'dictation';
  if (dom.appShell) dom.appShell.dataset.screen = next;
  document.body.dataset.screen = next;
  dom.dictationNav?.setAttribute('aria-current', next === 'dictation' ? 'page' : 'false');
  dom.settingsNav?.setAttribute('aria-current', next === 'onboarding' ? 'page' : 'false');
  if (focus) (next === 'dictation' ? dom.workspaceTitle : dom.setupTitleEl)?.focus();
}

export function setSetupScreenMode(mode) {
  state.setupScreenMode = mode === 'settings' ? 'settings' : 'onboarding';
  const settings = state.setupScreenMode === 'settings';
  if (dom.setupModeChipEl) dom.setupModeChipEl.textContent = settings ? 'ON THIS MAC' : 'WELCOME TO MURMUR';
  if (dom.setupTitleEl) dom.setupTitleEl.textContent = settings ? 'Setup & settings' : 'Make room for your voice.';
  if (dom.setupLeadEl) dom.setupLeadEl.textContent = settings
    ? 'A few preferences, just the way you like them.'
    : 'Three small checks, then you’re ready to speak. Your Mac does the transcription.';
  if (dom.setupStepsEl) dom.setupStepsEl.hidden = settings;
}

export function syncFlowForSetupReadiness() {
  if (state.hasInitialRoute) return;
  state.hasInitialRoute = true;
  if (dom.appShell) dom.appShell.dataset.initializing = 'false';
  const ready = !isNativeDesktopMode() || isNativeSetupReady();
  setSetupScreenMode(ready ? 'settings' : 'onboarding');
  setAppScreen(ready ? 'dictation' : 'onboarding', { focus: false });
}
