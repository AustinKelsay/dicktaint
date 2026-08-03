/** Platform detection and Tauri bridge helpers. */
import { state } from './state.js';

/**
 * @returns {boolean} Whether the runtime appears to be on macOS or iOS.
 */
export function isMacPlatform() {
  const source = [
    navigator.userAgentData?.platform,
    navigator.platform,
    navigator.userAgent
  ].filter(Boolean).join(' ');
  return /Mac|iPhone|iPad|iPod/i.test(source);
}

/** @returns {import('@tauri-apps/api/core').invoke | null} */
export function getTauriInvoke() {
  return window.__TAURI__?.core?.invoke
    || window.__TAURI__?.tauri?.invoke
    || null;
}

/** @returns {import('@tauri-apps/api/event').EventApi | null} */
export function getTauriEventApi() {
  return window.__TAURI__?.event || null;
}

/** @returns {'macos' | 'windows' | 'linux' | 'unknown'} */
export function detectDesktopOs() {
  const source = [
    state.currentDeviceProfile?.os,
    navigator.userAgentData?.platform,
    navigator.platform,
    navigator.userAgent
  ].filter(Boolean).join(' ');

  if (/mac|darwin/i.test(source)) return 'macos';
  if (/win/i.test(source)) return 'windows';
  if (/linux|x11/i.test(source)) return 'linux';
  return 'unknown';
}

/** @returns {boolean} */
export function isMobileUserAgent() {
  if (navigator.userAgentData?.mobile) return true;
  const ua = navigator.userAgent || '';
  return /Android|iPhone|iPad|iPod/i.test(ua);
}

/** @returns {boolean} */
export function isNativeDesktopMode() {
  return Boolean(getTauriInvoke()) && !isMobileUserAgent();
}

/** @returns {boolean} */
export function isFocusedMacDesktopMode() {
  return isNativeDesktopMode() && detectDesktopOs() === 'macos';
}

/** @returns {boolean} */
export function shouldUseTauriCommands() {
  return isFocusedMacDesktopMode();
}

/**
 * @param {unknown} error
 * @returns {string}
 */
export function getErrorMessage(error) {
  if (!error) return 'Unknown error';
  const normalize = (value) => {
    if (typeof value !== 'string') return '';
    const trimmed = value.trim();
    if (!trimmed) return '';
    if (trimmed === 'undefined' || trimmed === 'null' || trimmed === '[object Object]') return '';
    return trimmed;
  };

  const direct = normalize(error);
  if (direct) return direct;

  const message = normalize(error.message);
  if (message) return message;

  const nestedError = normalize(error.error);
  if (nestedError) return nestedError;

  try {
    const asJson = JSON.stringify(error);
    if (asJson && asJson !== '{}') return asJson;
  } catch {}
  const fallback = normalize(String(error));
  return fallback || 'Unknown error';
}
