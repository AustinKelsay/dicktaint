/** Web speech recognition restart timer helpers. */
import { state } from './state.js';
import { setUiMode, setStatus, syncControls } from './ui.js';

export function clearRestartTimer() {
  if (!state.restartTimer) return;
  clearTimeout(state.restartTimer);
  state.restartTimer = null;
}


export function scheduleRecognitionRestart() {
  clearRestartTimer();

  // Browser speech engines can end between utterances; auto-restart keeps hands-free flow.
  state.restartTimer = setTimeout(() => {
    if (!state.recognition || !state.shouldKeepDictating || state.isStartingDictation || state.isDictating) return;

    try {
      state.isStartingDictation = true;
      syncControls();
      state.recognition.start();
      setUiMode('loading');
      setStatus('Reconnecting dictation...', 'working');
    } catch {
      state.isStartingDictation = false;
      syncControls();
    }
  }, 250);
}

