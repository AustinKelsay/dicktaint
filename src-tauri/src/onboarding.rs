//! Assembles the dictation onboarding DTO from deep modules.
//!
//! Keeps catalog/settings/path logic in `models` while composing hotkey, insert,
//! audio, and whisper-cli readiness into one payload for the frontend gate.

use crate::audio::list_input_devices;
use crate::hotkey_overlay::{
    current_registered_hotkey, current_trigger_runtime_details, default_dictation_trigger,
    focused_field_insert_enabled, onboarding_runtime_details, resolve_effective_dictation_trigger,
    GlobalHotkeyState,
};
use crate::insert::focused_field_insert_permission_status;
use crate::models::{build_device_profile, build_model_options, resolve_active_model_path};
use crate::state::{
    resolve_background_ui_preferences, AppConfig, DictationOnboardingPayload, LocalModelState,
};
use crate::whisper_cli::{detect_whisper_cli_path, resolve_whisper_cli_path};

/// Builds the onboarding payload used by the desktop readiness gate.
pub(crate) fn build_onboarding_payload(
    config: &AppConfig,
    model_state: &LocalModelState,
    hotkey_state: &GlobalHotkeyState,
) -> Result<DictationOnboardingPayload, String> {
    let device = build_device_profile();
    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    let override_model_path = config
        .whisper_model_path_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let selected_model_path = if override_model_path.is_some() {
        override_model_path.clone()
    } else {
        settings.selected_model_path.clone()
    };
    // Capture owns the model-path rules, including custom selections and overrides.
    // The settings clone above releases its lock before the resolver reads them.
    let selected_model_exists = resolve_active_model_path(config, model_state).is_ok();
    let selected_model_id = if override_model_path.is_some() {
        Some("env-override".to_string())
    } else {
        settings.selected_model_id.clone()
    };
    let dictation_trigger = resolve_effective_dictation_trigger(&settings);
    let registered_trigger = current_registered_hotkey(hotkey_state).ok().flatten();
    let registered_runtime = current_trigger_runtime_details(hotkey_state).ok();
    let trigger_runtime = onboarding_runtime_details(
        dictation_trigger.as_deref(),
        registered_trigger.as_deref(),
        registered_runtime.as_ref(),
    );
    let list_selected_model_id = if override_model_path.is_some() {
        None
    } else {
        settings.selected_model_id.as_deref()
    };
    let models = build_model_options(
        &model_state.models_dir,
        device.total_memory_gb,
        list_selected_model_id,
    );
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let detected_whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path);
    let whisper_cli_available = detected_whisper_cli_path.is_some();
    let onboarding_required = !selected_model_exists || !whisper_cli_available;
    let focused_field_permission =
        focused_field_insert_permission_status(focused_field_insert_enabled(&settings));
    let background_ui_preferences = resolve_background_ui_preferences(&settings);
    let available_input_devices = list_input_devices();

    Ok(DictationOnboardingPayload {
        onboarding_required,
        selected_model_id,
        selected_model_path,
        selected_model_exists,
        available_input_devices,
        preferred_input_device: settings.preferred_input_device.clone(),
        dictation_trigger,
        default_dictation_trigger: default_dictation_trigger(),
        dictation_trigger_mode: trigger_runtime.mode.as_str().to_string(),
        dictation_trigger_status: trigger_runtime.status,
        dictation_trigger_permission_hint: trigger_runtime.permission_hint,
        pill_visibility_mode: background_ui_preferences
            .pill_visibility_mode
            .as_str()
            .to_string(),
        menu_bar_mode: background_ui_preferences.menu_bar_mode.as_str().to_string(),
        close_action: background_ui_preferences.close_action.as_str().to_string(),
        focused_field_insert_enabled: focused_field_insert_enabled(&settings),
        focused_field_insert_permission_granted: focused_field_permission.granted,
        focused_field_insert_permission_status: focused_field_permission.status,
        whisper_cli_available,
        whisper_cli_path: detected_whisper_cli_path.unwrap_or(configured_whisper_cli_path),
        models_dir: model_state.models_dir.to_string_lossy().to_string(),
        device,
        models,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::build_onboarding_payload;
    use crate::hotkey_overlay::GlobalHotkeyState;
    use crate::models::{load_local_settings, resolve_active_model_path};
    use crate::state::{AppConfig, LocalModelState};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    };

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "murmur-model-readiness-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn config(&self) -> AppConfig {
            // The executable is the OS boundary; no transcription is simulated here.
            let cli = self.0.join("whisper-cli");
            fs::write(
                &cli,
                "#!/bin/sh\nprintf 'usage: whisper-cli --model FILE\\n'\n",
            )
            .unwrap();
            fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
            AppConfig {
                whisper_model_path_override: None,
                whisper_cli_path_override: Some(cli.to_string_lossy().into_owned()),
                bundled_whisper_cli_path: None,
            }
        }

        fn saved_model(&self, id: &str, path: &std::path::Path) -> LocalModelState {
            let settings_path = self.0.join("settings.json");
            let settings = serde_json::json!({
                "selected_model_id": id,
                "selected_model_path": path,
                "preferred_input_device": "existing microphone",
                "dictation_trigger_enabled": false
            });
            fs::write(&settings_path, serde_json::to_vec(&settings).unwrap()).unwrap();
            LocalModelState {
                settings: Arc::new(Mutex::new(load_local_settings(&settings_path))),
                settings_path,
                models_dir: self.0.clone(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn existing_custom_model_is_ready_for_setup_and_capture_without_settings_changes() {
        let fixture = Fixture::new();
        let model = fixture.0.join("custom-local-model.bin");
        fs::write(&model, b"existing custom model file").unwrap();
        let state = fixture.saved_model("my-custom-model", &model);
        let saved_bytes = fs::read(&state.settings_path).unwrap();
        let config = fixture.config();

        assert_eq!(resolve_active_model_path(&config, &state).unwrap(), model);
        let payload =
            build_onboarding_payload(&config, &state, &GlobalHotkeyState::default()).unwrap();
        assert!(payload.selected_model_exists);
        assert!(!payload.onboarding_required);
        assert_eq!(
            payload.selected_model_id.as_deref(),
            Some("my-custom-model")
        );
        assert_eq!(payload.selected_model_path.as_deref(), model.to_str());
        assert_eq!(fs::read(&state.settings_path).unwrap(), saved_bytes);
        let saved = state.settings.lock().unwrap();
        assert_eq!(saved.selected_model_id.as_deref(), Some("my-custom-model"));
        assert_eq!(saved.selected_model_path.as_deref(), model.to_str());
        assert_eq!(
            saved.preferred_input_device.as_deref(),
            Some("existing microphone")
        );
    }

    #[test]
    fn environment_model_directory_is_rejected_by_setup_and_capture() {
        let fixture = Fixture::new();
        let directory = fixture.0.join("model-directory");
        fs::create_dir(&directory).unwrap();
        let state = fixture.saved_model("my-custom-model", &directory);
        let mut config = fixture.config();
        config.whisper_model_path_override = Some(directory.to_string_lossy().into_owned());

        assert!(resolve_active_model_path(&config, &state).is_err());
        let payload =
            build_onboarding_payload(&config, &state, &GlobalHotkeyState::default()).unwrap();
        assert!(!payload.selected_model_exists);
        assert!(payload.onboarding_required);
        assert_eq!(payload.selected_model_id.as_deref(), Some("env-override"));
    }

    #[test]
    fn saved_custom_model_directory_is_rejected_by_setup_and_capture() {
        let fixture = Fixture::new();
        let directory = fixture.0.join("model-directory");
        fs::create_dir(&directory).unwrap();
        let state = fixture.saved_model("my-custom-model", &directory);
        let config = fixture.config();

        assert!(resolve_active_model_path(&config, &state).is_err());
        let payload =
            build_onboarding_payload(&config, &state, &GlobalHotkeyState::default()).unwrap();
        assert!(!payload.selected_model_exists);
        assert!(payload.onboarding_required);
    }

    #[test]
    fn existing_environment_model_takes_precedence_without_replacing_the_saved_selection() {
        let fixture = Fixture::new();
        let model = fixture.0.join("external-model.bin");
        fs::write(&model, b"existing external model file").unwrap();
        let saved_model = fixture.0.join("unavailable-saved-model.bin");
        let state = fixture.saved_model("my-custom-model", &saved_model);
        let saved_bytes = fs::read(&state.settings_path).unwrap();
        let mut config = fixture.config();
        config.whisper_model_path_override = Some(format!("  {}  ", model.display()));

        assert_eq!(resolve_active_model_path(&config, &state).unwrap(), model);
        let payload =
            build_onboarding_payload(&config, &state, &GlobalHotkeyState::default()).unwrap();
        assert!(payload.selected_model_exists);
        assert!(!payload.onboarding_required);
        assert_eq!(payload.selected_model_id.as_deref(), Some("env-override"));
        assert_eq!(payload.selected_model_path.as_deref(), model.to_str());
        assert_eq!(fs::read(&state.settings_path).unwrap(), saved_bytes);
        assert_eq!(
            state
                .settings
                .lock()
                .unwrap()
                .selected_model_path
                .as_deref(),
            saved_model.to_str()
        );
    }

    #[test]
    fn missing_saved_or_environment_models_are_not_ready() {
        let fixture = Fixture::new();
        let missing_model = fixture.0.join("missing-model.bin");
        let state = fixture.saved_model("my-custom-model", &missing_model);
        let mut config = fixture.config();

        for path_override in [None, Some(missing_model.to_string_lossy().into_owned())] {
            config.whisper_model_path_override = path_override;
            assert!(resolve_active_model_path(&config, &state).is_err());
            let payload =
                build_onboarding_payload(&config, &state, &GlobalHotkeyState::default()).unwrap();
            assert!(!payload.selected_model_exists);
            assert!(payload.onboarding_required);
        }
    }

    #[test]
    fn catalog_model_must_be_complete_before_setup_and_capture_accept_it() {
        let fixture = Fixture::new();
        let model = fixture.0.join("ggml-tiny.en.bin");
        fs::write(&model, b"partial download").unwrap();
        let state = fixture.saved_model("tiny-en", &model);
        let config = fixture.config();

        assert!(resolve_active_model_path(&config, &state).is_err());
        let partial =
            build_onboarding_payload(&config, &state, &GlobalHotkeyState::default()).unwrap();
        assert!(!partial.selected_model_exists);
        assert!(partial.onboarding_required);

        // A sparse 40 MB fixture clears the existing catalog size heuristic.
        // This readiness check does not establish transcription success.
        fs::OpenOptions::new()
            .write(true)
            .open(&model)
            .unwrap()
            .set_len(40_000_000)
            .unwrap();
        assert_eq!(resolve_active_model_path(&config, &state).unwrap(), model);
        let complete =
            build_onboarding_payload(&config, &state, &GlobalHotkeyState::default()).unwrap();
        assert!(complete.selected_model_exists);
        assert!(!complete.onboarding_required);
    }
}
