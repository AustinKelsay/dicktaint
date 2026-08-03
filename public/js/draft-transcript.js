/**
 * Draft transcript text helpers (leaf — no history/transcript cycle).
 *
 * Layering: draft-transcript ← history | transcript ← events / native / web
 */
import { dom } from './dom-elements.js';
import { state } from './state.js';

/**
 * Replaces the draft transcript with the given text.
 * @param {string} text
 */
export function setDraftTranscriptText(text) {
  state.currentDraftText = String(text || '').trim();
  dom.transcriptInput.value = state.currentDraftText;
}

/**
 * Appends trimmed text to the draft transcript.
 * @param {string} text
 * @returns {boolean} Whether anything was appended.
 */
export function appendToDraftTranscript(text) {
  const trimmed = String(text || '').trim();
  if (!trimmed) return false;
  state.currentDraftText = `${state.currentDraftText} ${trimmed}`.trim();
  dom.transcriptInput.value = state.currentDraftText;
  return true;
}
