/**
 * Model select helpers (leaf — breaks ui.js ↔ onboarding/models.js cycle).
 *
 * Layering: model-selection ← ui | models | events
 */
import { dom } from '../dom-elements.js';
import { state } from '../state.js';

/**
 * @returns {object | null} Currently selected catalog model, if any.
 */
export function getSelectedDictationModel() {
  const selectedId = (dom.dictationModelSelect?.value || '').trim();
  if (!selectedId) return null;
  return state.dictationModels.find((model) => model.id === selectedId) || null;
}

/**
 * Updates the install button label for the current selection.
 */
export function updateModelActionLabels() {
  if (!dom.installDictationModelBtn) return;

  const selected = getSelectedDictationModel();
  if (!selected) {
    dom.installDictationModelBtn.textContent = 'Download + Use';
    return;
  }

  const isCurrent = Boolean(state.currentOnboarding?.selected_model_exists)
    && state.currentOnboarding?.selected_model_id === selected.id;

  if (!selected.installed) {
    dom.installDictationModelBtn.textContent = 'Download + Use';
    return;
  }

  dom.installDictationModelBtn.textContent = isCurrent ? 'Using Now' : 'Use Installed';
}
