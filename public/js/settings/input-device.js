/**
 * Microphone input device selection.
 */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';
import { getTauriInvoke, isFocusedMacDesktopMode, getErrorMessage } from '../platform.js';
import { refreshDictationOnboarding } from '../onboarding/refresh.js';
import { setStatus, syncControls } from '../ui.js';

/**
 * @param {string} message
 * @param {string} [tone]
 */
export function setDictationInputStatus(message, tone = 'neutral') {
  if (!dom.dictationInputStatusEl) return;
  dom.dictationInputStatusEl.textContent = message;
  dom.dictationInputStatusEl.dataset.tone = tone;
}

/**
 * @param {Array<object>} devices
 * @param {string | null | undefined} selectedDeviceName
 */
export function renderInputDeviceOptions(devices, selectedDeviceName) {
  if (!dom.dictationInputSelectEl) return;

  const available = Array.isArray(devices) ? devices : [];
  dom.dictationInputSelectEl.innerHTML = '';

  const systemOption = document.createElement('option');
  systemOption.value = '';
  const systemDefault = available.find((device) => device?.is_default)?.name;
  systemOption.textContent = systemDefault
    ? `System Default (${systemDefault})`
    : 'System Default';
  dom.dictationInputSelectEl.appendChild(systemOption);

  for (const device of available) {
    const name = String(device?.name || '').trim();
    if (!name) continue;
    const option = document.createElement('option');
    option.value = name;
    option.textContent = device?.is_default ? `${name} (Default)` : name;
    dom.dictationInputSelectEl.appendChild(option);
  }

  dom.dictationInputSelectEl.value = selectedDeviceName || '';

  if (selectedDeviceName) {
    setDictationInputStatus(`Preferred microphone: ${selectedDeviceName}.`, 'ok');
  } else if (systemDefault) {
    setDictationInputStatus(`Using the system default microphone: ${systemDefault}.`, 'neutral');
  } else {
    setDictationInputStatus('Using the system default microphone.', 'neutral');
  }
}

/**
 * Persists preferred input device and refreshes onboarding.
 * @param {string} deviceName
 */
export async function savePreferredInputDevice(deviceName) {
  const tauriInvoke = getTauriInvoke();
  if (!tauriInvoke || !isFocusedMacDesktopMode()) return;

  try {
    state.isSavingInputDevice = true;
    syncControls();
    const normalized = String(deviceName || '').trim();
    const saved = await tauriInvoke('set_preferred_input_device', {
      deviceName: normalized || null
    });
    state.preferredInputDevice = saved || null;
    state.currentOnboarding = await refreshDictationOnboarding({ quietStatus: true });
    setStatus(
      state.preferredInputDevice
        ? `Preferred microphone saved: ${state.preferredInputDevice}.`
        : 'Microphone reset to system default.',
      'ok'
    );
  } catch (error) {
    const details = getErrorMessage(error);
    renderInputDeviceOptions(state.currentOnboarding?.available_input_devices, state.preferredInputDevice);
    setDictationInputStatus(`Could not save microphone: ${details}`, 'error');
    setStatus(`Could not save microphone: ${details}`, 'error');
  } finally {
    state.isSavingInputDevice = false;
    syncControls();
  }
}
