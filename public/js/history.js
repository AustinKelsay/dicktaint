/** Recent dictation history list and actions. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { DICTATION_HISTORY_LIMIT } from './constants.js';
import { getErrorMessage } from './platform.js';
import { setStatus } from './ui.js';
import { appendToDraftTranscript } from './draft-transcript.js';

/**
 * @returns {string} Unique history entry id.
 */
export function nextDictationHistoryId() {
  state.dictationHistorySeq += 1;
  return `dict-${Date.now()}-${state.dictationHistorySeq}`;
}

/**
 * Pushes a transcript chunk onto recent history and re-renders the list.
 * @param {string} chunk
 * @param {string} [source]
 */
export function pushDictationHistory(chunk, source = 'native') {
  const trimmed = String(chunk || '').trim();
  if (!trimmed) return;
  state.dictationHistory = [
    {
      id: nextDictationHistoryId(),
      text: trimmed,
      source,
      createdAt: new Date().toISOString()
    },
    ...state.dictationHistory
  ].slice(0, DICTATION_HISTORY_LIMIT);
  renderDictationHistory();
}

/**
 * @param {string} historyId
 * @returns {object | null}
 */
export function findDictationHistoryEntry(historyId) {
  const id = String(historyId || '').trim();
  if (!id) return null;
  return state.dictationHistory.find((entry) => entry.id === id) || null;
}

/**
 * @param {string} value
 * @returns {string}
 */
export function formatHistoryTimestamp(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

/**
 * @param {string} source
 * @returns {string}
 */
export function historySourceLabel(source) {
  const value = String(source || '').trim().toLowerCase();
  if (value === 'web') return 'WEB';
  if (value === 'native' || value === 'native-event') return 'DESKTOP';
  return 'DICTATION';
}

/**
 * @param {string} text
 * @returns {Promise<boolean>}
 */
export async function copyTextToClipboard(text) {
  const trimmed = String(text || '').trim();
  if (!trimmed) return false;

  const tauriClipboard = window.__TAURI__?.clipboardManager
    || window.TAURI?.clipboardManager
    || window.__TAURI__?.clipboard
    || window.__TAURI__?.plugins?.clipboard
    || null;
  if (typeof tauriClipboard?.writeText === 'function') {
    await tauriClipboard.writeText(trimmed);
    return true;
  }

  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(trimmed);
    return true;
  }

  console.warn('Clipboard fallback unavailable: no tauri clipboard API or navigator.clipboard.writeText.');
  return false;
}

/**
 * Runs a history row action (reinsert / copy).
 * @param {string} historyAction
 * @param {string} historyId
 * @returns {Promise<boolean>}
 */
export async function runDictationHistoryAction(historyAction, historyId) {
  const action = String(historyAction || '').trim();
  const id = String(historyId || '').trim();
  const entry = findDictationHistoryEntry(id);
  if (!entry) {
    setStatus('That history entry is no longer available.', 'error');
    return false;
  }

  if (action === 'reinsert') {
    if (appendToDraftTranscript(entry.text)) {
      dom.transcriptInput.focus();
      setStatus('Reinserted previous dictation into transcript.', 'ok');
      return true;
    }
    return false;
  }

  if (action === 'copy') {
    try {
      const copied = await copyTextToClipboard(entry.text);
      if (copied) {
        setStatus('Copied dictation entry to clipboard.', 'ok');
      } else {
        setStatus('Could not copy dictation entry to clipboard.', 'error');
      }
      return copied;
    } catch (error) {
      const details = getErrorMessage(error);
      setStatus(`Could not copy dictation entry: ${details}`, 'error');
      return false;
    }
  }

  setStatus(`Unknown history action: ${action || '(empty)'}.`, 'error');
  return false;
}

/** Renders the recent dictation history list. */
export function renderDictationHistory() {
  if (!dom.dictationHistorySection || !dom.dictationHistoryListEl || !dom.dictationHistoryEmptyEl) return;
  dom.dictationHistorySection.hidden = false;
  const hasHistory = state.dictationHistory.length > 0;
  dom.dictationHistoryEmptyEl.hidden = hasHistory;
  if (dom.clearDictationHistoryBtn) dom.clearDictationHistoryBtn.disabled = !hasHistory;

  dom.dictationHistoryListEl.innerHTML = '';
  if (!hasHistory) return;

  for (const entry of state.dictationHistory) {
    const item = document.createElement('li');
    item.className = 'dictation-history-item';

    const text = document.createElement('p');
    text.className = 'dictation-history-text';
    text.textContent = entry.text;

    const meta = document.createElement('p');
    meta.className = 'dictation-history-meta';
    const stamp = formatHistoryTimestamp(entry.createdAt);
    const source = historySourceLabel(entry.source);
    meta.textContent = stamp ? `${stamp} • ${source}` : source;

    const actions = document.createElement('div');
    actions.className = 'dictation-history-actions';

    const reuseBtn = document.createElement('button');
    reuseBtn.type = 'button';
    reuseBtn.className = 'ghost quiet';
    reuseBtn.textContent = 'Reinsert';
    reuseBtn.dataset.historyAction = 'reinsert';
    reuseBtn.dataset.historyId = entry.id;

    const copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'ghost quiet';
    copyBtn.textContent = 'Copy';
    copyBtn.dataset.historyAction = 'copy';
    copyBtn.dataset.historyId = entry.id;

    actions.appendChild(reuseBtn);
    actions.appendChild(copyBtn);
    item.appendChild(text);
    item.appendChild(meta);
    item.appendChild(actions);
    dom.dictationHistoryListEl.appendChild(item);
  }
}
