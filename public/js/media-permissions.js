/**
 * Shared microphone permission probe for native and web dictation paths.
 *
 * Layering: media-permissions ← native-dictation | web-speech | events
 * Keep web-speech.js focused on browser SpeechRecognition only.
 */
import { state } from './state.js';
import { isFocusedMacDesktopMode } from './platform.js';

/**
 * Probes getUserMedia once so the browser/OS grants mic access before capture.
 * No-op on focused macOS desktop (native path owns the mic).
 */
export async function ensureMicrophoneAccess() {
  if (isFocusedMacDesktopMode()) return;
  if (!navigator.mediaDevices?.getUserMedia) return;

  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  state.hasMicrophoneAccess = true;
  for (const track of stream.getTracks()) {
    track.stop();
  }
}
