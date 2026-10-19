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
  syncDraftMeta();
}

/**
 * Describes the draft length for the draft footer.
 * @param {string} text
 * @returns {string}
 */
export function describeDraftLength(text) {
  const words = String(text || '').trim().split(/\s+/u).filter(Boolean).length;
  if (!words) return 'Empty draft';
  return words === 1 ? '1 word' : `${words} words`;
}

/** Refreshes the draft footer from the current textarea contents. */
export function syncDraftMeta() {
  if (!dom.draftMeta) return;
  dom.draftMeta.textContent = describeDraftLength(dom.transcriptInput?.value ?? state.currentDraftText);
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
  syncDraftMeta();
  return true;
}
