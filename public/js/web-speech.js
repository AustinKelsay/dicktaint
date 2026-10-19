/** Browser SpeechRecognition dictation path (mic probe lives in media-permissions.js). */
import { state } from './state.js';
import { isFocusedMacDesktopMode } from './platform.js';

export { ensureMicrophoneAccess } from './media-permissions.js';

export function describeSpeechError(errorCode) {
  if (errorCode === 'not-allowed' || errorCode === 'service-not-allowed') {
    if (state.hasMicrophoneAccess) {
      return isFocusedMacDesktopMode()
        ? 'speech recognition isn’t allowed. In System Settings > Privacy & Security > Speech Recognition, allow Murmur, then reopen it.'
        : 'speech recognition isn’t allowed in this browser. Allow it for this site and try again.';
    }

    return isFocusedMacDesktopMode()
      ? 'microphone access isn’t allowed. In System Settings > Privacy & Security > Microphone, allow Murmur, then reopen it.'
      : 'microphone access isn’t allowed in this browser. Allow it for this site and try again.';
  }

  if (errorCode === 'audio-capture') {
    return 'no microphone was found. Check your microphone connection and selected input, then try again.';
  }

  if (errorCode === 'no-speech') {
    return 'the mic opened but no speech was detected. Try again, a little closer to the mic.';
  }

  if (errorCode === 'network') {
    return 'the speech service couldn’t be reached. Check your connection and try again.';
  }

  if (errorCode === 'aborted') {
    return 'speech recognition stopped unexpectedly.';
  }

  if (errorCode === 'language-not-supported') {
    return 'this language isn’t supported for speech recognition.';
  }

  return errorCode || 'unknown';
}


export function isFatalSpeechError(errorCode) {
  return ['not-allowed', 'service-not-allowed', 'audio-capture', 'network', 'language-not-supported'].includes(errorCode);
}
