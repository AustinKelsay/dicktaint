/**
 * Shared display labels (leaf — avoids waveform ↔ onboarding coupling).
 */

/**
 * Human-readable model name without a trailing "(Selected)" suffix.
 * @param {{ display_name?: string } | null | undefined} model
 * @returns {string}
 */
export function modelDisplayName(model) {
  return String(model?.display_name || '').replace(/\s+\(Selected\)$/u, '').trim();
}
