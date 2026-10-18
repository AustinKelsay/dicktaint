/** Required setup capabilities; optional grants never gate in-app capture. */
import { state } from './state.js';
import { isFocusedMacDesktopMode } from './platform.js';
export function isNativeSetupReady() {
  return !isFocusedMacDesktopMode() || Boolean(state.nativeDictationModelReady
    && state.macosPermissions?.supported && state.macosPermissions.microphone === 'authorized');
}
