//! macOS menu bar tray runtime, labels, and menu event handling.

use crate::dictation_session::{
    cancel_if_active, current_active_session_id, is_benign_session_error, start, stop,
};
use crate::state::{
    BackendDictationStatus, MAIN_TRAY_ID, MenuBarMode, TRAY_MENU_FORCE_STOP_ID, TRAY_MENU_OPEN_ID,
    TRAY_MENU_QUIT_ID, TRAY_MENU_STATUS_ID, TRAY_MENU_TOGGLE_ID, TrayRuntimeState, TrayState,
};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

use super::background_ui::{
    current_backend_dictation_status, current_background_ui_preferences, main_window_is_visible,
    should_show_tray_icon, show_main_window,
};
use super::emit_dictation_state;

pub(crate) fn tray_primary_action_label(status: BackendDictationStatus) -> &'static str {
    match status {
        BackendDictationStatus::Listening => "Stop + Transcribe",
        BackendDictationStatus::Processing => "Transcribing...",
        BackendDictationStatus::Idle | BackendDictationStatus::Error => "Start Dictation",
    }
}

pub(crate) fn tray_primary_action_enabled(status: BackendDictationStatus) -> bool {
    status != BackendDictationStatus::Processing
}

pub(crate) fn tray_force_stop_enabled(status: BackendDictationStatus) -> bool {
    status == BackendDictationStatus::Listening
}

pub(crate) fn tray_title_for_backend_status(status: BackendDictationStatus) -> &'static str {
    match status {
        BackendDictationStatus::Idle => "DT",
        BackendDictationStatus::Listening => "REC",
        BackendDictationStatus::Processing => "...",
        BackendDictationStatus::Error => "ERR",
    }
}

pub(crate) fn destroy_macos_tray_runtime(app: &tauri::AppHandle) -> Result<(), String> {
    let tray_state = app.state::<TrayState>();
    let mut guard = tray_state
        .runtime
        .lock()
        .map_err(|_| "Failed to lock tray runtime state".to_string())?;
    *guard = None;
    Ok(())
}

pub(crate) fn handle_tray_menu_event(app: &tauri::AppHandle, menu_id: &tauri::menu::MenuId) {
    if menu_id == TRAY_MENU_STATUS_ID {
        return;
    }

    if menu_id == TRAY_MENU_OPEN_ID {
        show_main_window(app);
        return;
    }

    if menu_id == TRAY_MENU_FORCE_STOP_ID {
        if let Err(error) = cancel_if_active(app) {
            emit_dictation_state(
                app,
                "error",
                Some(error.clone()),
                None,
                current_active_session_id(app).ok().flatten(),
            );
            log::warn!("Tray force-stop failed: {error}");
        }
        return;
    }

    if menu_id == TRAY_MENU_QUIT_ID {
        if let Err(error) = cancel_if_active(app) {
            log::warn!("Tray quit mic teardown failed: {error}");
        }
        app.exit(0);
        return;
    }

    if menu_id != TRAY_MENU_TOGGLE_ID {
        return;
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = match current_backend_dictation_status(&handle) {
            Ok(BackendDictationStatus::Listening) => stop(handle.clone()).await.map(|_| ()),
            Ok(BackendDictationStatus::Processing) => Ok(()),
            Ok(BackendDictationStatus::Idle | BackendDictationStatus::Error) => {
                start(&handle).map(|_| ())
            }
            Err(error) => Err(error),
        };

        if let Err(error) = result {
            if !is_benign_session_error(&error) {
                log::warn!("Tray dictation action failed: {error}");
                emit_dictation_state(&handle, "error", Some(error), None, None);
            }
        }
    });
}

pub(crate) fn ensure_macos_tray_runtime(
    app: &tauri::AppHandle,
    status: BackendDictationStatus,
) -> Result<(), String> {
    let tray_state = app.state::<TrayState>();
    let mut guard = tray_state
        .runtime
        .lock()
        .map_err(|_| "Failed to lock tray runtime state".to_string())?;
    if guard.is_some() {
        return Ok(());
    }

    let status_item = MenuItemBuilder::with_id(
        TRAY_MENU_STATUS_ID,
        format!("Status: {}", status.tray_label()),
    )
    .enabled(false)
    .build(app)
    .map_err(|e| format!("Failed to build tray status item: {e}"))?;
    let open_item = MenuItemBuilder::with_id(TRAY_MENU_OPEN_ID, "Open dicktaint")
        .build(app)
        .map_err(|e| format!("Failed to build tray open item: {e}"))?;
    let toggle_item =
        MenuItemBuilder::with_id(TRAY_MENU_TOGGLE_ID, tray_primary_action_label(status))
            .enabled(tray_primary_action_enabled(status))
            .build(app)
            .map_err(|e| format!("Failed to build tray toggle item: {e}"))?;
    let force_stop_item =
        MenuItemBuilder::with_id(TRAY_MENU_FORCE_STOP_ID, "Force Stop Microphone")
            .enabled(tray_force_stop_enabled(status))
            .build(app)
            .map_err(|e| format!("Failed to build tray force-stop item: {e}"))?;
    let quit_item = MenuItemBuilder::with_id(TRAY_MENU_QUIT_ID, "Quit dicktaint")
        .build(app)
        .map_err(|e| format!("Failed to build tray quit item: {e}"))?;

    let menu = MenuBuilder::new(app)
        .item(&status_item)
        .item(&open_item)
        .item(&toggle_item)
        .item(&force_stop_item)
        .separator()
        .item(&quit_item)
        .build()
        .map_err(|e| format!("Failed to build macOS tray menu: {e}"))?;

    let tray_icon = TrayIconBuilder::with_id(MAIN_TRAY_ID)
        .menu(&menu)
        .title(tray_title_for_backend_status(status))
        .show_menu_on_left_click(true)
        .tooltip("dicktaint")
        .on_menu_event(|app, event: tauri::menu::MenuEvent| {
            handle_tray_menu_event(app, event.id());
        })
        .build(app)
        .map_err(|e| format!("Failed to create macOS tray icon: {e}"))?;

    *guard = Some(TrayRuntimeState {
        tray_icon,
        status_item,
        toggle_item,
        force_stop_item,
    });
    Ok(())
}

pub(crate) fn sync_macos_tray(app: &tauri::AppHandle, status: BackendDictationStatus) -> Result<(), String> {
    let preferences = current_background_ui_preferences(app)?;
    if preferences.menu_bar_mode == MenuBarMode::Off {
        return destroy_macos_tray_runtime(app);
    }

    ensure_macos_tray_runtime(app, status)?;

    let tray_state = app.state::<TrayState>();
    let guard = tray_state
        .runtime
        .lock()
        .map_err(|_| "Failed to lock tray runtime state".to_string())?;
    let Some(runtime) = guard.as_ref() else {
        return Ok(());
    };

    runtime
        .status_item
        .set_text(format!("Status: {}", status.tray_label()))
        .map_err(|e| format!("Failed to update tray status text: {e}"))?;
    runtime
        .toggle_item
        .set_text(tray_primary_action_label(status))
        .map_err(|e| format!("Failed to update tray action text: {e}"))?;
    runtime
        .toggle_item
        .set_enabled(tray_primary_action_enabled(status))
        .map_err(|e| format!("Failed to update tray action enabled state: {e}"))?;
    runtime
        .force_stop_item
        .set_enabled(tray_force_stop_enabled(status))
        .map_err(|e| format!("Failed to update tray force-stop enabled state: {e}"))?;
    runtime
        .tray_icon
        .set_tooltip(Some(format!("dicktaint: {}", status.tray_label())))
        .map_err(|e| format!("Failed to update tray tooltip: {e}"))?;
    runtime
        .tray_icon
        .set_title(Some(tray_title_for_backend_status(status)))
        .map_err(|e| format!("Failed to update tray title: {e}"))?;
    runtime
        .tray_icon
        .set_visible(should_show_tray_icon(
            preferences,
            main_window_is_visible(app),
        ))
        .map_err(|e| format!("Failed to update tray visibility: {e}"))?;
    Ok(())
}
