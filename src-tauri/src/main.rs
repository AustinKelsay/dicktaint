//! dicktaint Tauri backend entry point.

mod audio;
mod commands;
mod hotkey_overlay;
mod insert;
mod models;
mod state;
mod transcribe;
mod whisper_cli;

use commands::{
    cancel_native_dictation, cancel_native_dictation_if_active, clear_dictation_trigger,
    delete_dictation_model, get_dictation_onboarding, get_dictation_trigger,
    insert_text_into_focused_field, install_dictation_model, open_whisper_setup_page,
    set_close_action, set_dictation_trigger, set_focused_field_insert_enabled,
    set_menu_bar_mode, set_pill_visibility_mode, set_preferred_input_device,
    start_native_dictation, stop_native_dictation,
};
use hotkey_overlay::{
    GlobalHotkeyState,
    apply_registered_hotkey, create_pill_overlay_windows, current_background_ui_preferences,
    dispatch_backend_hotkey_action, resolve_effective_dictation_trigger, should_start_hidden,
    show_main_window, sync_background_ui,
};
use models::{load_local_settings, resolve_local_paths};
use state::{
    AppConfig, BackendHotkeyAction, BackgroundUiPreferences, CloseAction, DictationState,
    LocalModelState, MenuBarMode, PillVisibilityMode,
};
#[cfg(target_os = "macos")]
use state::TrayState;
use whisper_cli::resolve_bundled_whisper_cli_path;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri_plugin_global_shortcut::ShortcutState;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg(test)]
mod tests {
    use crate::audio::{
        ordered_input_device_candidate_names, probe_input_stream_activity,
        wait_for_non_silent_input, InputStreamProbeOutcome,
        should_focus_main_window_for_microphone_prompt,
    };
    use crate::hotkey_overlay::{
        default_dictation_trigger, focused_field_insert_enabled,
        macos_listener_disable_should_dispatch_stop, macos_tap_disable_should_dispatch_stop,
        normalize_dictation_trigger, onboarding_runtime_details,
        pill_should_be_visible_for_backend_state,
        resolve_effective_dictation_trigger, runtime_details_for_trigger,
        tray_force_stop_enabled,
        tray_primary_action_enabled, tray_primary_action_label, HotkeyDeliveryMode,
    };
    use crate::state::{
        resolve_background_ui_preferences, BackendDictationStatus, CloseAction, LocalSettings,
        MenuBarMode, PillVisibilityMode,
    };
    use crate::transcribe::{
        analyze_audio_signal, audio_signal_is_too_quiet, normalize_audio_gain, quiet_audio_error,
        resample_linear,
    };
    use crate::whisper_cli::{preferred_whisper_cli_names, whisper_help_text_looks_valid};
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[cfg(target_os = "macos")]
    use objc2_av_foundation::AVAuthorizationStatus;


    #[test]
    fn resample_linear_returns_same_when_rate_matches() {
        let source = vec![0.0_f32, 0.5, -0.5, 1.0];
        let out = resample_linear(&source, 16_000, 16_000);
        assert_eq!(out, source);
    }

    #[test]
    fn resample_linear_produces_output_when_rate_changes() {
        let source = vec![0.0_f32, 1.0, 0.0, -1.0];
        let out = resample_linear(&source, 8_000, 16_000);
        assert!(out.len() > source.len());
        assert!(out.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn analyze_audio_signal_reports_peak_rms_and_duration() {
        let samples = vec![0.0_f32, 0.25, -0.5, 0.5];
        let stats = analyze_audio_signal(&samples, 8_000);
        assert!((stats.peak_abs - 0.5).abs() < 0.0001);
        assert!(stats.rms > 0.0);
        assert!(stats.duration_secs > 0.0);
    }

    #[test]
    fn quiet_audio_detection_flags_near_silent_capture() {
        let samples = vec![0.0002_f32; 16_000];
        let stats = analyze_audio_signal(&samples, 16_000);
        assert!(audio_signal_is_too_quiet(stats));
        assert!(quiet_audio_error(stats, "MacBook Pro Microphone").contains("too quiet"));
    }

    #[test]
    fn normalize_audio_gain_boosts_quiet_but_valid_audio() {
        let samples = vec![0.01_f32, -0.02, 0.03, -0.04];
        let stats = analyze_audio_signal(&samples, 16_000);
        assert!(!audio_signal_is_too_quiet(stats));
        let boosted = normalize_audio_gain(samples.clone(), stats);
        let boosted_peak = boosted
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        let original_peak = samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        assert!(boosted_peak > original_peak);
        assert!(boosted_peak <= 0.85);
    }

    #[test]
    fn silent_stream_probe_accepts_zeroed_frames_when_frames_exist() {
        let samples = Arc::new(Mutex::new(vec![0.0_f32; 4096]));
        let outcome = probe_input_stream_activity(
            &samples,
            0,
            16_000,
            "Austin's AirPods",
            Duration::ZERO,
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(outcome, InputStreamProbeOutcome::SilentFrames);
        wait_for_non_silent_input(&samples, 0, 16_000, "Austin's AirPods").unwrap();
    }

    #[test]
    fn silent_stream_probe_accepts_nonzero_frames() {
        let samples = Arc::new(Mutex::new(vec![0.0_f32, 0.02, -0.01, 0.0]));
        let outcome = probe_input_stream_activity(
            &samples,
            0,
            16_000,
            "MacBook Pro Microphone",
            Duration::ZERO,
            Duration::ZERO,
        )
        .unwrap();
        assert_eq!(outcome, InputStreamProbeOutcome::NonSilentFrames);
        wait_for_non_silent_input(&samples, 0, 16_000, "MacBook Pro Microphone").unwrap();
    }

    #[test]
    fn silent_stream_probe_rejects_missing_frames() {
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let error = probe_input_stream_activity(
            &samples,
            0,
            16_000,
            "MacBook Pro Microphone",
            Duration::ZERO,
            Duration::ZERO,
        )
        .unwrap_err();
        assert!(error.contains("did not deliver any audio frames"));
    }

    #[test]
    fn whisper_help_text_accepts_real_help_snippet() {
        let stdout = "usage: whisper-cli [options] file0.wav\n  -m FNAME  model path";
        assert!(whisper_help_text_looks_valid(stdout, ""));
    }

    #[test]
    fn whisper_help_text_rejects_placeholder_snippet() {
        let stderr =
            "Bundled whisper-cli placeholder. Replace with a real whisper-cli sidecar binary.";
        assert!(!whisper_help_text_looks_valid("", stderr));
    }

    #[test]
    fn normalize_dictation_trigger_accepts_valid_combo() {
        assert_eq!(
            normalize_dictation_trigger("cmdorctrl + shift + d").unwrap(),
            "CmdOrCtrl+Shift+D".to_string()
        );
    }

    #[test]
    fn normalize_dictation_trigger_accepts_fn_key() {
        assert_eq!(normalize_dictation_trigger("fn").unwrap(), "Fn".to_string());
        assert_eq!(
            normalize_dictation_trigger("globe").unwrap(),
            "Fn".to_string()
        );
    }

    #[test]
    fn normalize_dictation_trigger_rejects_fn_with_modifiers() {
        assert!(normalize_dictation_trigger("Shift+Fn").is_err());
    }

    #[test]
    fn normalize_dictation_trigger_rejects_missing_modifier() {
        assert!(normalize_dictation_trigger("D").is_err());
    }

    #[test]
    fn normalize_dictation_trigger_rejects_multiple_main_keys() {
        assert!(normalize_dictation_trigger("Ctrl+K+J").is_err());
    }

    #[test]
    fn resolve_effective_trigger_defaults_when_unset() {
        let settings = LocalSettings::default();
        assert_eq!(
            resolve_effective_dictation_trigger(&settings),
            Some(default_dictation_trigger())
        );
    }

    #[test]
    fn resolve_effective_trigger_honors_explicit_disable() {
        let settings = LocalSettings {
            dictation_trigger_enabled: Some(false),
            ..LocalSettings::default()
        };
        assert_eq!(resolve_effective_dictation_trigger(&settings), None);
    }

    #[test]
    fn resolve_effective_trigger_uses_saved_value() {
        let settings = LocalSettings {
            dictation_trigger: Some("CmdOrCtrl+Shift+K".to_string()),
            dictation_trigger_enabled: Some(true),
            ..LocalSettings::default()
        };
        assert_eq!(
            resolve_effective_dictation_trigger(&settings),
            Some("CmdOrCtrl+Shift+K".to_string())
        );
    }

    #[test]
    fn focused_field_insert_defaults_to_disabled() {
        let settings = LocalSettings::default();
        assert!(!focused_field_insert_enabled(&settings));
    }

    #[test]
    fn focused_field_insert_uses_explicit_enabled_setting() {
        let settings = LocalSettings {
            focused_field_insert_enabled: Some(true),
            ..LocalSettings::default()
        };
        assert!(focused_field_insert_enabled(&settings));
    }

    #[test]
    fn runtime_details_report_fn_permission_fallback() {
        let runtime =
            runtime_details_for_trigger(Some("Fn"), HotkeyDeliveryMode::FocusedWindowHold);
        assert_eq!(runtime.mode.as_str(), "focused-window-hold");
        assert!(runtime.status.contains("focused"));
        assert!(runtime.permission_hint.is_some());
    }

    #[test]
    fn onboarding_runtime_prefers_registered_global_fn_state() {
        let registered_runtime =
            runtime_details_for_trigger(Some("Fn"), HotkeyDeliveryMode::GlobalHold);
        let runtime = onboarding_runtime_details(Some("Fn"), Some("Fn"), Some(&registered_runtime));
        assert_eq!(runtime.mode.as_str(), "global-hold");
        assert!(runtime.status.contains("anywhere"));
    }

    #[test]
    fn onboarding_runtime_falls_back_when_fn_runtime_is_unknown() {
        let runtime = onboarding_runtime_details(Some("Fn"), None, None);
        assert_eq!(runtime.mode.as_str(), "focused-window-hold");
        assert!(runtime.status.contains("focused"));
        assert!(runtime.permission_hint.is_some());
    }

    #[test]
    fn background_ui_preferences_default_to_active_only_always_and_hide_to_tray() {
        let preferences = resolve_background_ui_preferences(&LocalSettings::default());
        assert_eq!(
            preferences.pill_visibility_mode,
            PillVisibilityMode::ActiveOnly
        );
        assert_eq!(preferences.menu_bar_mode, MenuBarMode::Always);
        assert_eq!(preferences.close_action, CloseAction::HideToTray);
    }

    #[test]
    fn background_ui_preferences_force_quit_when_menu_bar_is_off() {
        let preferences = resolve_background_ui_preferences(&LocalSettings {
            menu_bar_mode: Some("off".to_string()),
            close_action: Some("hide-to-tray".to_string()),
            ..LocalSettings::default()
        });
        assert_eq!(preferences.menu_bar_mode, MenuBarMode::Off);
        assert_eq!(preferences.close_action, CloseAction::Quit);
    }

    #[test]
    fn pill_visibility_modes_map_expected_states() {
        assert!(!pill_should_be_visible_for_backend_state(
            BackendDictationStatus::Idle,
            PillVisibilityMode::ActiveOnly,
            false,
        ));
        assert!(pill_should_be_visible_for_backend_state(
            BackendDictationStatus::Listening,
            PillVisibilityMode::ActiveOnly,
            false,
        ));
        assert!(pill_should_be_visible_for_backend_state(
            BackendDictationStatus::Processing,
            PillVisibilityMode::ActiveOnly,
            false,
        ));
        assert!(!pill_should_be_visible_for_backend_state(
            BackendDictationStatus::Error,
            PillVisibilityMode::ActiveOnly,
            false,
        ));
        assert!(pill_should_be_visible_for_backend_state(
            BackendDictationStatus::Error,
            PillVisibilityMode::ActiveOnly,
            true,
        ));
        assert!(!pill_should_be_visible_for_backend_state(
            BackendDictationStatus::Listening,
            PillVisibilityMode::Off,
            true,
        ));
        assert!(pill_should_be_visible_for_backend_state(
            BackendDictationStatus::Idle,
            PillVisibilityMode::Always,
            false,
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn listener_disable_dispatches_stop_when_fn_was_down() {
        assert!(macos_listener_disable_should_dispatch_stop(true));
        assert!(!macos_listener_disable_should_dispatch_stop(false));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn tap_disable_dispatches_stop_when_fn_was_down() {
        assert!(macos_tap_disable_should_dispatch_stop(true));
        assert!(!macos_tap_disable_should_dispatch_stop(false));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn tray_mapping_reflects_backend_status() {
        assert_eq!(BackendDictationStatus::Idle.tray_label(), "Idle");
        assert_eq!(BackendDictationStatus::Listening.tray_label(), "Listening");
        assert_eq!(
            BackendDictationStatus::Processing.tray_label(),
            "Transcribing"
        );
        assert_eq!(BackendDictationStatus::Error.tray_label(), "Error");

        assert_eq!(
            tray_primary_action_label(BackendDictationStatus::Idle),
            "Start Dictation"
        );
        assert_eq!(
            tray_primary_action_label(BackendDictationStatus::Listening),
            "Stop + Transcribe"
        );
        assert_eq!(
            tray_primary_action_label(BackendDictationStatus::Processing),
            "Transcribing..."
        );
        assert!(!tray_primary_action_enabled(
            BackendDictationStatus::Processing
        ));
        assert!(tray_force_stop_enabled(BackendDictationStatus::Listening));
        assert!(!tray_force_stop_enabled(BackendDictationStatus::Idle));
    }

    #[test]
    fn preferred_default_input_uses_default_handle_and_dedupes_names() {
        let ordered = ordered_input_device_candidate_names(
            Some("MacBook Pro Microphone"),
            Some("MacBook Pro Microphone"),
            &[
                "MacBook Pro Microphone".to_string(),
                "USB Mic".to_string(),
                "MacBook Pro Microphone".to_string(),
            ],
        );

        assert_eq!(
            ordered,
            vec![
                "MacBook Pro Microphone".to_string(),
                "USB Mic".to_string()
            ]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn microphone_permission_prompt_only_focuses_for_not_determined_status() {
        assert!(should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::NotDetermined
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Authorized
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Denied
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Restricted
        ));
    }

    #[test]
    fn preferred_whisper_cli_names_include_generic_fallback() {
        let names = preferred_whisper_cli_names();
        assert!(names.iter().any(|name| name == "whisper-cli"));
    }
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init();

    let whisper_model_path_override = std::env::var("WHISPER_MODEL_PATH").ok();
    let whisper_cli_path_override = std::env::var("WHISPER_CLI_PATH").ok();

    let builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    dispatch_backend_hotkey_action(app, BackendHotkeyAction::Toggle);
                }
            })
            .build(),
    );

    let app = builder
        .setup(move |app| {
            let bundled_whisper_cli_path = resolve_bundled_whisper_cli_path(app.handle());
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                format!(
                    "Failed to resolve Tauri app data directory while initializing local dictation paths: {e}"
                )
            })?;
            let (models_dir, settings_path) = resolve_local_paths(&app_data_dir).map_err(|e| {
                format!(
                    "Failed to initialize local dictation model paths under {}: {e}",
                    app_data_dir.display()
                )
            })?;
            let initial_settings = load_local_settings(&settings_path);
            let initial_dictation_trigger = resolve_effective_dictation_trigger(&initial_settings);

            app.manage(AppConfig {
                whisper_model_path_override: whisper_model_path_override.clone(),
                whisper_cli_path_override: whisper_cli_path_override.clone(),
                bundled_whisper_cli_path,
            });
            app.manage(LocalModelState {
                settings_path,
                models_dir,
                settings: Arc::new(Mutex::new(initial_settings)),
            });
            app.manage(DictationState::default());
            app.manage(GlobalHotkeyState::default());
            #[cfg(target_os = "macos")]
            app.manage(TrayState::default());

            if let Err(error) = apply_registered_hotkey(
                app.handle(),
                app.state::<GlobalHotkeyState>().inner(),
                initial_dictation_trigger.as_deref(),
            ) {
                log::warn!("Failed to apply initial global hotkey: {error}");
            }

            if should_start_hidden() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            if let Err(error) = create_pill_overlay_windows(app.handle()) {
                log::warn!("Failed to create pill overlay windows: {error}");
            }

            sync_background_ui(app.handle());

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let preferences = current_background_ui_preferences(window.app_handle()).unwrap_or(
                    BackgroundUiPreferences {
                        pill_visibility_mode: PillVisibilityMode::ActiveOnly,
                        menu_bar_mode: MenuBarMode::Always,
                        close_action: CloseAction::HideToTray,
                    },
                );
                if preferences.close_action == CloseAction::HideToTray
                    && preferences.menu_bar_mode != MenuBarMode::Off
                {
                    api.prevent_close();
                    let _ = window.hide();
                    sync_background_ui(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_dictation_onboarding,
            get_dictation_trigger,
            set_dictation_trigger,
            clear_dictation_trigger,
            set_pill_visibility_mode,
            set_menu_bar_mode,
            set_close_action,
            set_preferred_input_device,
            set_focused_field_insert_enabled,
            open_whisper_setup_page,
            insert_text_into_focused_field,
            install_dictation_model,
            delete_dictation_model,
            start_native_dictation,
            stop_native_dictation,
            cancel_native_dictation
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => show_main_window(app_handle),
        tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
            if let Err(error) = cancel_native_dictation_if_active(app_handle) {
                log::warn!("Failed to cancel active dictation during app exit: {error}");
            }
        }
        _ => {}
    });
}
