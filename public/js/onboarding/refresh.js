/**
 * Onboarding reload seam — breaks models/input-device ↔ onboarding/index cycles.
 *
 * index.js registers the real loader; install/delete/input-device call through here.
 */

/** @type {(opts?: { quietStatus?: boolean }) => Promise<unknown>} */
let onboardingLoader = async () => {
  throw new Error('Dictation onboarding loader is not registered yet.');
};

/**
 * Registers the canonical onboarding loader (call once from onboarding/index.js).
 * @param {(opts?: { quietStatus?: boolean }) => Promise<unknown>} loader
 */
export function registerDictationOnboardingLoader(loader) {
  if (typeof loader !== 'function') {
    throw new Error('Dictation onboarding loader must be a function.');
  }
  onboardingLoader = loader;
}

/**
 * Reloads onboarding via the registered loader.
 * @param {{ quietStatus?: boolean }} [opts]
 * @returns {Promise<unknown>}
 */
export async function refreshDictationOnboarding(opts = {}) {
  return onboardingLoader(opts);
}
