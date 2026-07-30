/** Draft transcript text and chunk append pipeline. */
import { state } from './state.js';
import { maybeInsertTranscriptIntoFocusedField } from './settings/focused-field-insert.js';
import { pushDictationHistory } from './history.js';
import {
  setDraftTranscriptText,
  appendToDraftTranscript
} from './draft-transcript.js';

export { setDraftTranscriptText, appendToDraftTranscript };

/**
 * Attempts to record a native session as committed.
 * Returns false when a duplicate session id is detected; null/empty ids return true.
 * @param {string | null} sessionId
 * @returns {boolean}
 */
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

/**
 * @param {unknown} sessionId
 * @returns {string | null}
 */
export function normalizeNativeSessionId(sessionId) {
  if (sessionId === null || sessionId === undefined) return null;
  const normalized = String(sessionId).trim();
  return normalized || null;
}

/**
 * Appends a finished transcript chunk to draft + history (+ optional focused insert).
 * @param {string} chunk
 * @param {{ source?: string, nativeSessionId?: string | null }} [options]
 * @returns {boolean}
 */
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
