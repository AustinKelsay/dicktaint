/** Draft transcript text and chunk append pipeline. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { maybeInsertTranscriptIntoFocusedField } from './settings/focused-field-insert.js';
import { pushDictationHistory } from './history.js';

export function tryCommitNativeSession(sessionId) {
  if (!sessionId) return true;
  if (state.committedNativeSessionIds.has(sessionId)) return false;
  state.committedNativeSessionIds.add(sessionId);
  if (state.committedNativeSessionIds.size > 64) {
    const keep = Array.from(state.committedNativeSessionIds).slice(-32);
    state.committedNativeSessionIds = new Set(keep);
  }
  return true;
}


export function normalizeNativeSessionId(sessionId) {
  if (sessionId === null || sessionId === undefined) return null;
  const normalized = String(sessionId).trim();
  return normalized || null;
}


export function setDraftTranscriptText(text) {
  state.currentDraftText = String(text || '').trim();
  dom.transcriptInput.value = state.currentDraftText;
}


export function appendToDraftTranscript(text) {
  const trimmed = String(text || '').trim();
  if (!trimmed) return false;
  state.currentDraftText = `${state.currentDraftText} ${trimmed}`.trim();
  dom.transcriptInput.value = state.currentDraftText;
  return true;
}


export function appendTranscriptChunk(chunk, { source = 'native', nativeSessionId = null } = {}) {
  const trimmed = String(chunk || '').trim();
  if (!trimmed) return false;
  const isNativeSource = source === 'native' || source === 'native-event';
  if (isNativeSource && state.rejectNextNativeAppend) {
    return false;
  }
  if (isNativeSource && state.nativeSessionIdToIgnore && (nativeSessionId === null || nativeSessionId === state.nativeSessionIdToIgnore)) {
    return false;
  }
  if (!tryCommitNativeSession(nativeSessionId)) return false;
  appendToDraftTranscript(trimmed);
  state.rejectNextNativeAppend = false;
  pushDictationHistory(trimmed, source);
  void maybeInsertTranscriptIntoFocusedField(trimmed);
  return true;
}

