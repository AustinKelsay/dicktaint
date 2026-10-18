export { isNativeSetupReady } from './readiness.js';
/** macOS permission facts, explicit actions, and required setup readiness. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage } from './platform.js';
import { syncControls, syncFlowForSetupReadiness } from './ui.js';
import { renderPermissionGuidance } from './settings/hotkey-ui.js';
import { applyDictationHotkeyPayload } from './settings/hotkeys.js';

let refreshInFlight = null;
let revision = 0;
function updatePermissionUi() {
  renderPermissionGuidance();
  syncControls();
  // A quiet focus return must preserve the current settings location and session.
  if (!state.isDictating && !state.isStartingDictation && state.setupScreenMode !== 'settings') {
    syncFlowForSetupReadiness();
  }
}
function acceptSnapshot(snapshot) {
  const valid = snapshot?.supported === true && ['not_determined', 'authorized', 'denied', 'restricted', 'unavailable'].includes(snapshot.microphone)
    && typeof snapshot.accessibility === 'boolean' && typeof snapshot.input_monitoring === 'boolean';
  if (!valid) throw new Error('Permission status is unavailable. Retry the check.');
  state.macosPermissions = snapshot;
  state.macosPermissionError = '';
}
export async function refreshMacosPermissions({ rearm = false } = {}) {
  if (!isFocusedMacDesktopMode()) return null;
  if (refreshInFlight) return refreshInFlight;
  const current = ++revision;
  refreshInFlight = (async () => {
    try {
      const invoke = getTauriInvoke();
      const snapshot = await invoke('get_macos_permissions');
      if (current !== revision) return null;
      acceptSnapshot(snapshot);
      if (rearm && !state.isDictating && !state.isStartingDictation && !state.nativeStopRequestInFlight) {
        // Backend independently guards pending starts and active capture.
        try {
          await invoke('recheck_macos_permissions');
          applyDictationHotkeyPayload(await invoke('get_dictation_trigger'));
        } catch (error) {
          state.macosPermissionError = getErrorMessage(error);
        }
      }
      return snapshot;
    } catch (error) {
      if (current === revision) {
        state.macosPermissions = null;
        state.macosPermissionError = getErrorMessage(error);
      }
      return null;
    } finally {
      refreshInFlight = null;
      updatePermissionUi();
    }
  })();
  return refreshInFlight;
}
export async function requestMicrophonePermission() {
  if (state.isRequestingMicrophone) return;
  ++revision;
  state.isRequestingMicrophone = true;
  updatePermissionUi();
  try {
    acceptSnapshot(await getTauriInvoke()('request_microphone_permission'));
  } catch (error) {
    state.macosPermissions = null;
    state.macosPermissionError = getErrorMessage(error);
  } finally {
    state.isRequestingMicrophone = false;
    updatePermissionUi();
  }
}
export async function openMacosPermissionSettings(permission) {
  if (!['microphone', 'accessibility', 'input_monitoring'].includes(permission)) return;
  try {
    await getTauriInvoke()('open_macos_permission_settings', { permission });
  } catch (error) {
    state.macosPermissionError = getErrorMessage(error);
    updatePermissionUi();
  }
}
export function initPermissionListeners() {
  const permissionAction = (event) => {
    const target = event.target instanceof Element ? event.target.closest('button[data-permission-action]') : null;
    if (!target) return;
    if (target.dataset.permissionAction === 'allow') void requestMicrophonePermission();
    else if (target.dataset.permissionAction === 'settings') void openMacosPermissionSettings(target.dataset.permission);
    else void refreshMacosPermissions({ rearm: true });
  };
  dom.dictationPermissionListEl?.addEventListener('click', permissionAction);
  dom.requiredSetup?.addEventListener('click', permissionAction);
  const recheck = () => { if (document.visibilityState !== 'hidden') void refreshMacosPermissions({ rearm: true }); };
  window.addEventListener('focus', recheck);
  document.addEventListener('visibilitychange', recheck);
}
