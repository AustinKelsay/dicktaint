/**
 * Frontend ESM layering (acyclic).
 *
 * Leaves (no app feature imports):
 *   constants, state, dom-elements, platform, labels, media-permissions, draft-transcript
 *   hotkey-logic, background-ui-controls, model-selection, waveform, speech-runtime, refresh
 *
 * Mid:
 *   history, transcript, hotkey-ui, hotkeys, background-ui, focused-field-insert,
 *   input-device, onboarding/models, web-speech, native-dictation
 *
 * Top:
 *   ui, onboarding/index, events, app.js
 *
 * Cycles are broken by leaf extraction + onboarding refresh registration
 * (see onboarding/refresh.js). Do not reintroduce mutual imports among peers.
 */
export {};
