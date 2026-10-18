/**
 * Frontend ESM imports, from leaves toward orchestration.
 *
 * Leaves: constants, state, dom-elements, platform, labels, media-permissions,
 * draft-transcript, readiness, hotkey-logic, background-ui-controls,
 * model-selection, waveform, speech-runtime, onboarding/refresh.
 *
 * Presentation: navigation, workspace-presentation, hotkey-ui → ui.
 * Features: ui → history, transcript, settings saves, onboarding/models.
 * Orchestration: permissions, native-dictation, web-speech, onboarding/index,
 * then events → app.js. history imports ui for status feedback.
 *
 * navigation owns screen choice, not session state. workspace-presentation
 * renders capability facts without navigating. onboarding/refresh registers
 * the loader to avoid a model-actions ↔ onboarding import cycle.
 */
export {};
