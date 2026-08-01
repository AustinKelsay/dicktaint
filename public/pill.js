/**
 * Overlay pill window runtime.
 * Loaded by `public/pill.html` (Tauri overlay). Classic script — no ESM bundler.
 * Event name strings mirror `public/js/constants.js` (separate document; cannot share imports).
 */
(function () {
  const pill = document.getElementById('pill');
  const click = document.getElementById('click');
  const waveformBars = Array.from(document.querySelectorAll('.waveform span'));
  const DICTATION_AUDIO_LEVEL_EVENT = 'dictation:audio-level';
  const PILL_STATUS_EVENT = 'dicktaint://pill-status';

  let state = 'idle';
  let currentSessionId = null;
  /** Guards click/hotkey start/stop so overlapping invokes cannot race. */
  let actionInFlight = false;

  /**
   * Clamps a numeric level into the inclusive [0, 1] range.
   * @param {unknown} value
   * @returns {number}
   */
  function clampLevel(value) {
    const numeric = Number(value);
    if (!Number.isFinite(numeric)) return 0;
    return Math.max(0, Math.min(1, numeric));
  }

  /**
   * Compresses a source bar array into a fixed count of peak values.
   * @param {unknown} source
   * @param {number} count
   * @returns {number[]}
   */
  function compressBars(source, count) {
    const values = Array.isArray(source) ? source : [];
    if (!values.length) {
      return Array.from({ length: count }, (_, index) => 0.08 + ((index % 2) * 0.08));
    }

    const bars = [];
    for (let index = 0; index < count; index += 1) {
      const start = Math.floor((index * values.length) / count);
      const end = Math.max(start + 1, Math.floor(((index + 1) * values.length) / count));
      let peak = 0;
      for (let sourceIndex = start; sourceIndex < end && sourceIndex < values.length; sourceIndex += 1) {
        peak = Math.max(peak, clampLevel(values[sourceIndex]));
      }
      bars.push(peak);
    }
    return bars;
  }

  /**
   * Maps a live audio level to a CSS audio-state token while listening.
   * @param {number} level
   * @returns {'idle' | 'low' | 'ready' | 'hot'}
   */
  function audioStateForLevel(level) {
    if (state !== 'listening') return 'idle';
    if (level < 0.18) return 'low';
    if (level > 0.92) return 'hot';
    return 'ready';
  }

  /** Clears live-level CSS vars and resets waveform bars. */
  function resetMeter() {
    pill.dataset.audioState = state === 'error' ? 'error' : 'idle';
    pill.style.setProperty('--live-level', '0');
    for (const bar of waveformBars) {
      bar.style.setProperty('--bar-level', '0.080');
    }
  }

  /**
   * Shows or hides the overlay body via opacity.
   * @param {boolean} visible
   */
  function setVisible(visible) {
    document.body.dataset.visible = visible ? 'true' : 'false';
    document.body.style.opacity = visible ? '1' : '0';
  }

  /**
   * Maps overlay status event states onto pill DOM states.
   * @param {unknown} value
   * @returns {'listening' | 'processing' | 'error' | 'idle'}
   */
  function normalizeOverlayStatusState(value) {
    const normalized = String(value || '').toLowerCase();
    if (normalized === 'live') return 'listening';
    if (normalized === 'working') return 'processing';
    if (normalized === 'error') return 'error';
    return 'idle';
  }

  /**
   * Applies a live audio level and optional bar peaks to the waveform.
   * @param {unknown} level
   * @param {unknown} bars
   */
  function setMeter(level, bars) {
    const normalizedLevel = clampLevel(level);
    pill.dataset.audioState = audioStateForLevel(normalizedLevel);
    pill.style.setProperty('--live-level', normalizedLevel.toFixed(3));
    const normalizedBars = compressBars(bars, waveformBars.length);
    for (let index = 0; index < waveformBars.length; index += 1) {
      waveformBars[index].style.setProperty('--bar-level', normalizedBars[index].toFixed(3));
    }
  }

  /**
   * Updates pill visual state and visibility.
   * @param {string} s
   * @param {{ visible?: boolean }} [options]
   */
  function setState(s, options = {}) {
    const visible = options.visible !== false && s !== 'idle';
    if (s === 'error') {
      state = 'error';
      pill.setAttribute('data-state', 'error');
      setVisible(visible);
      resetMeter();
      setTimeout(() => {
        if (state === 'error') {
          state = 'idle';
          pill.setAttribute('data-state', 'idle');
          currentSessionId = null;
          resetMeter();
          setVisible(false);
        }
      }, 1400);
      return;
    }
    state = s;
    pill.setAttribute('data-state', s);
    setVisible(visible);
    if (state !== 'listening') {
      currentSessionId = state === 'processing' ? currentSessionId : null;
      resetMeter();
    } else {
      setMeter(0, []);
    }
  }

  /** @returns {((cmd: string, ...args: unknown[]) => Promise<unknown>) | null} */
  function getInvoke() {
    return window.__TAURI__?.core?.invoke ?? window.__TAURI__?.tauri?.invoke ?? null;
  }

  /**
   * @param {unknown} error
   * @returns {boolean}
   */
  function isDictationAlreadyRunningError(error) {
    const message = String(error?.message || error || '').toLowerCase();
    return message.includes('dictation already running');
  }

  click.addEventListener('click', async () => {
    if (state === 'processing' || actionInFlight) return;
    const invoke = getInvoke();
    if (!invoke) return;
    if (state === 'idle') {
      actionInFlight = true;
      try {
        await invoke('start_native_dictation');
        setState('listening');
      } catch (e) {
        console.error(e);
        // Concurrent start while already running: keep/restore listening, not error.
        if (isDictationAlreadyRunningError(e)) setState('listening');
        else setState('error');
      } finally {
        actionInFlight = false;
      }
    } else if (state === 'listening') {
      actionInFlight = true;
      setState('processing');
      try {
        await invoke('stop_native_dictation');
      } catch (e) {
        console.error(e);
        setState('error');
      } finally {
        actionInFlight = false;
      }
    }
  });

  const ev = window.__TAURI__?.event;
  if (ev?.listen) {
    ev.listen('dictation:state-changed', ({ payload }) => {
      if (payload?.session_id !== null && payload?.session_id !== undefined) {
        currentSessionId = String(payload.session_id);
      }
      setState(payload?.state ?? 'idle');
    }).catch(() => {});
    ev.listen(PILL_STATUS_EVENT, ({ payload }) => {
      setState(normalizeOverlayStatusState(payload?.state), {
        visible: payload?.visible !== false
      });
    }).catch(() => {});
    ev.listen(DICTATION_AUDIO_LEVEL_EVENT, ({ payload }) => {
      const payloadSessionId = payload?.session_id === null || payload?.session_id === undefined
        ? null
        : String(payload.session_id);
      if (payloadSessionId && currentSessionId && payloadSessionId !== currentSessionId) return;
      if (state !== 'listening') return;
      setMeter(payload?.level, payload?.bars);
    }).catch(() => {});
    ev.listen('dictation:hotkey-triggered', () => {
      if (state === 'processing' || actionInFlight) return;
      click.click();
    }).catch(() => {});
  }

  setState('idle', { visible: false });
})();
