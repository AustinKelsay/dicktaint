/** Owns screen choice. Readiness chooses the initial route; quiet checks never navigate. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { isNativeSetupReady } from './readiness.js';
import { isNativeDesktopMode, isFocusedMacDesktopMode } from './platform.js';

export function setAppScreen(screen, { focus = true } = {}) {
  if (focus) {
    state.hasInitialRoute = true;
    if (dom.appShell) dom.appShell.dataset.initializing = 'false';
  }
  const next = screen === 'dictation' ? 'dictation' : 'onboarding';
  if (dom.onboardingScreen) dom.onboardingScreen.hidden = next !== 'onboarding';
  if (dom.dictationScreen) dom.dictationScreen.hidden = next !== 'dictation';
  if (dom.appShell) dom.appShell.dataset.screen = next;
  document.body.dataset.screen = next;
  dom.dictationNav?.setAttribute('aria-current', next === 'dictation' ? 'page' : 'false');
  dom.settingsNav?.setAttribute('aria-current', next === 'onboarding' ? 'page' : 'false');
  if (focus) (next === 'dictation' ? dom.workspaceTitle : dom.setupTitleEl)?.focus();
}

/** First run shows only the required checks; Settings shows every option and hides checks that already pass. */
export function syncSetupSections() {
  const settings = state.setupScreenMode === 'settings';
  if (dom.requiredSetup) dom.requiredSetup.hidden = !isNativeDesktopMode() || (settings && isNativeSetupReady());
  if (dom.optionalSettingsEl) dom.optionalSettingsEl.hidden = !settings;
  if (dom.setupLeadEl) dom.setupLeadEl.hidden = settings;
  if (dom.onboardingContinueBtn) dom.onboardingContinueBtn.hidden = settings;
}

export function setSetupScreenMode(mode) {
  state.setupScreenMode = mode === 'settings' ? 'settings' : 'onboarding';
  if (dom.setupTitleEl) dom.setupTitleEl.textContent = state.setupScreenMode === 'settings' ? 'Settings' : 'Make room for your voice.';
  syncSetupSections();
}

export function syncFlowForSetupReadiness() {
  if (state.hasInitialRoute || !state.onboardingCheckComplete
    || (isFocusedMacDesktopMode() && !state.permissionCheckComplete)) return;
  state.hasInitialRoute = true;
  if (dom.appShell) dom.appShell.dataset.initializing = 'false';
  const ready = !isNativeDesktopMode() || isNativeSetupReady();
  setSetupScreenMode(ready ? 'settings' : 'onboarding');
  setAppScreen(ready ? 'dictation' : 'onboarding', { focus: false });
}
