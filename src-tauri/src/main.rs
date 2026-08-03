//! dicktaint Tauri backend entry point.

mod audio;
mod commands;
mod dictation_session;
mod hotkey_overlay;
mod insert;
mod models;
mod onboarding;
mod state;
mod transcribe;
mod whisper_cli;

use commands::{
    cancel_native_dictation, clear_dictation_trigger, delete_dictation_model,
    get_dictation_onboarding, get_dictation_trigger, insert_text_into_focused_field,
    install_dictation_model, open_whisper_setup_page, set_close_action, set_dictation_trigger,
    set_focused_field_insert_enabled, set_menu_bar_mode, set_pill_visibility_mode,
    set_preferred_input_device, start_native_dictation, stop_native_dictation,
};
use dictation_session::cancel_if_active;
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
            if let Err(error) = cancel_if_active(app_handle) {
                log::warn!("Failed to cancel active dictation during app exit: {error}");
            }
        }
        _ => {}
    });
}
