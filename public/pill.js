const pillEl = document.getElementById('pill');
const pillTextEl = document.getElementById('pillText');
const PILL_STATUS_EVENT = 'dicktaint://pill-status';

function normalizeState(state) {
  const value = String(state || '').toLowerCase();
  if (['working', 'live', 'ok', 'error', 'idle'].includes(value)) {
    return value;
  }
  return 'idle';
}

function applyStatus(payload) {
  if (!pillEl || !pillTextEl) return;
  const state = normalizeState(payload?.state);
  const message = String(payload?.message || '').trim() || 'Hold fn to dictate';
  const visible = payload?.visible !== false;

  pillEl.dataset.state = state;
  pillTextEl.textContent = message;
  document.body.style.opacity = visible ? '1' : '0';
}

async function initOverlayListener() {
  const tauriEvent = window.__TAURI__?.event;
  if (typeof tauriEvent?.listen !== 'function') return;
  await tauriEvent.listen(PILL_STATUS_EVENT, (event) => {
    applyStatus(event?.payload || {});
  });
}

applyStatus({ state: 'idle', message: 'Hold fn to dictate', visible: true });
initOverlayListener().catch(() => {});
