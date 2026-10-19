# Model Management

## Status Snapshot

- Date: 2026-10-18
- model catalog and local lifecycle are implemented in Rust; setup and capture share active-model path validation

## Purpose

Define current model recommendation, install/delete, and persistence behavior.

## Scope

In scope:

- built-in model catalog semantics
- recommendation ranking behavior
- install/delete contract
- persistence strategy

Out of scope:

- external model sources beyond current hard-coded origin

## Source Anchors

- `src-tauri/src/main.rs` (thin entry)
- `src-tauri/src/commands.rs`
- `src-tauri/src/hotkey_overlay/mod.rs`
- `src-tauri/src/state.rs`
- `public/app.js` (ESM entry)
- `public/js/`

## Contract

Model catalog currently includes 12 ids:

- `tiny-en`, `tiny`, `base-en`, `base`, `small-en`, `small`, `medium-en`, `medium`, `large-v1`, `large-v2`, `large-v3`, `turbo`

Recommendation ranking:

1. compute fit level by RAM threshold
2. prefer higher fit level
3. then prefer higher `recommended_ram_gb`
4. then prefer larger model size tie-break

Install flow (`install_dictation_model`):

1. validate model id
2. verify `whisper-cli` availability
3. create model directory if needed
4. download model from Hugging Face if missing/incomplete, validating the downloaded size before selection
5. persist selected model id + path

Delete flow (`delete_dictation_model`):

1. delete target model file if present
2. if deleted model is selected, pick best installed fallback
3. clear selection if no fallback exists
4. persist updates

Persistence paths:

- settings: `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/dictation-settings.json`
- models: `$HOME/Library/Application Support/com.plebdev.dicktaint/.dicktaint/whisper-models/`

Readiness and upgrade continuity:

- onboarding and capture both use `resolve_active_model_path`; the selected-model readiness flag follows the same rules
- known catalog models must be files that pass the existing size heuristic; this does not prove model content or successful transcription
- an existing saved custom model file remains usable even when its id is outside the catalog
- `WHISPER_MODEL_PATH` takes precedence, trims surrounding whitespace, must point to a file, and does not replace the saved selection
- missing files and directories are not usable models
- the Murmur display rename retains the legacy application storage and settings schema

Write safety:

- settings writes are atomic via temp-file + rename flow

## Verification

Re-verify after model lifecycle changes:

1. `bun run test:rust`
2. desktop onboarding smoke for install/use/delete
3. confirm selected model fallback behavior after deleting active model

## Related Docs

- [`NATIVE_DESKTOP_DICTATION.md`](NATIVE_DESKTOP_DICTATION.md)
- [`CONFIG_AND_PATH_RESOLUTION.md`](CONFIG_AND_PATH_RESOLUTION.md)
- [`../context/USER_FLOWS.md`](../context/USER_FLOWS.md)
