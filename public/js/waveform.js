/** Dictation waveform visualization. */
import { dom } from './dom-elements.js';
import { state } from './state.js';
import { DICTATION_WAVEFORM_BAR_COUNT } from './constants.js';

export { modelDisplayName } from './labels.js';

export function clampAudioLevel(value) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return 0;
  return Math.max(0, Math.min(1, numeric));
}


export function defaultLiveAudioBars(count = DICTATION_WAVEFORM_BAR_COUNT) {
  return Array.from({ length: count }, (_, index) => {
    const distance = Math.abs(index - ((count - 1) / 2));
    return Math.max(0.08, 0.18 - (distance * 0.016));
  });
}


export function fallbackLiveAudioBars(level, count = DICTATION_WAVEFORM_BAR_COUNT) {
  const normalized = clampAudioLevel(level);
  return Array.from({ length: count }, (_, index) => {
    const phase = ((index % 4) + 1) / 4;
    return Math.max(0.08, Math.min(1, (normalized * phase * 0.9) + 0.08));
  });
}


export function normalizeLiveAudioBars(rawBars, level, count = DICTATION_WAVEFORM_BAR_COUNT) {
  const source = Array.isArray(rawBars) ? rawBars : [];
  if (!source.length) return fallbackLiveAudioBars(level, count);

  const normalized = [];
  for (let index = 0; index < count; index += 1) {
    const sourceIndex = Math.floor((index * source.length) / count);
    normalized.push(clampAudioLevel(source[sourceIndex]));
  }
  return normalized;
}


export function audioStateForLevel(level, mode = document.body?.dataset?.mode || 'idle') {
  if (mode !== 'listening') return mode === 'error' ? 'error' : 'idle';
  if (level < 0.18) return 'low';
  if (level > 0.92) return 'hot';
  return 'ready';
}


export function setInlineStyleProperty(target, property, value) {
  if (!target?.style) return;
  if (typeof target.style.setProperty === 'function') {
    target.style.setProperty(property, value);
    return;
  }
  target.style[property] = value;
}


export function updateDictationWaveform(level = 0, bars = defaultLiveAudioBars(), mode = document.body?.dataset?.mode || 'idle') {
  state.liveAudioLevel = clampAudioLevel(level);
  state.liveAudioBars = normalizeLiveAudioBars(bars, state.liveAudioLevel, DICTATION_WAVEFORM_BAR_COUNT);

  if (dom.dictationWaveformEl) {
    dom.dictationWaveformEl.dataset.audioState = audioStateForLevel(state.liveAudioLevel, mode);
    setInlineStyleProperty(dom.dictationWaveformEl, '--live-level', state.liveAudioLevel.toFixed(3));
  }

  for (let index = 0; index < dom.dictationWaveformBars.length; index += 1) {
    const value = state.liveAudioBars[index] ?? 0.08;
    setInlineStyleProperty(dom.dictationWaveformBars[index], '--bar-level', value.toFixed(3));
  }

  if (dom.dictationWaveformLevelEl) {
    const audioState = audioStateForLevel(state.liveAudioLevel, mode);
    dom.dictationWaveformLevelEl.dataset.audioState = audioState;
    dom.dictationWaveformLevelEl.textContent = audioState === 'idle'
      ? 'Mic level: waiting...'
      : (audioState === 'error'
        ? 'Mic level unavailable.'
        : (audioState === 'low'
          ? 'Mic level: low'
          : (audioState === 'hot' ? 'Mic level: hot' : 'Mic level: good')));
  }
}


export function resetDictationWaveform(mode = document.body?.dataset?.mode || 'idle') {
  updateDictationWaveform(0, defaultLiveAudioBars(), mode);
}

