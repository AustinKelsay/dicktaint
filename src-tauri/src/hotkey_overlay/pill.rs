//! Overlay pill window creation, sizing, and status sync.

use crate::state::{
    BackendDictationStatus, BackgroundUiPreferences, CloseAction, MenuBarMode, PILL_STATUS_EVENT,
    PillStatusPayload, PillVisibilityMode,
};
#[cfg(target_os = "macos")]
use crate::state::{
    MAX_PILL_WINDOWS, PILL_WINDOW_BASE_WIDTH, PILL_WINDOW_BOTTOM_MARGIN, PILL_WINDOW_HEIGHT,
    PILL_WINDOW_LABEL_PREFIX, PILL_WINDOW_MIN_WIDTH,
};
use tauri::{Emitter, Manager};

use super::background_ui::current_background_ui_preferences;
use super::trigger::{
    HotkeyDeliveryMode, GlobalHotkeyState, current_registered_hotkey,
    current_trigger_runtime_details, default_dictation_trigger,
};

pub(crate) fn active_hotkey_label(app: &tauri::AppHandle) -> String {
    let hotkey_state = app.state::<GlobalHotkeyState>();
    let trigger = current_registered_hotkey(hotkey_state.inner())
        .ok()
        .flatten()
        .unwrap_or_else(default_dictation_trigger);
    if trigger == "Fn" {
        "Fn / Globe".to_string()
    } else {
        trigger
    }
}

pub(crate) fn idle_pill_message(app: &tauri::AppHandle) -> String {
    let hotkey_state = app.state::<GlobalHotkeyState>();
    let runtime = current_trigger_runtime_details(hotkey_state.inner()).unwrap_or_default();
    let label = active_hotkey_label(app);
    match runtime.mode {
        HotkeyDeliveryMode::GlobalHold | HotkeyDeliveryMode::FocusedWindowHold => {
            format!("Hold {label} to dictate")
        }
        HotkeyDeliveryMode::GlobalToggle => format!("Press {label} to dictate"),
        HotkeyDeliveryMode::Disabled => "Hotkey disabled".to_string(),
    }
}

pub(crate) fn pill_should_be_visible_for_backend_state(
    status: BackendDictationStatus,
    mode: PillVisibilityMode,
    has_error: bool,
) -> bool {
    match mode {
        PillVisibilityMode::Off => false,
        PillVisibilityMode::Always => !matches!(status, BackendDictationStatus::Error) || has_error,
        PillVisibilityMode::ActiveOnly => {
            matches!(
                status,
                BackendDictationStatus::Listening | BackendDictationStatus::Processing
            ) || (status == BackendDictationStatus::Error && has_error)
        }
    }
}

pub(crate) fn emit_pill_status(
    app: &tauri::AppHandle,
    message: impl Into<String>,
    state: impl Into<String>,
    visible: bool,
) {
    app.emit(
        PILL_STATUS_EVENT,
        PillStatusPayload {
            message: message.into(),
            state: state.into(),
            visible,
        },
    )
    .ok();
}

pub(crate) fn sync_pill_for_backend_state(
    app: &tauri::AppHandle,
    status: BackendDictationStatus,
    error: Option<&str>,
) {
    let hotkey_state = app.state::<GlobalHotkeyState>();
    let runtime = current_trigger_runtime_details(hotkey_state.inner()).unwrap_or_default();
    let preferences = current_background_ui_preferences(app).unwrap_or(BackgroundUiPreferences {
        pill_visibility_mode: PillVisibilityMode::ActiveOnly,
        menu_bar_mode: MenuBarMode::Always,
        close_action: CloseAction::HideToTray,
    });
    let label = active_hotkey_label(app);
    let has_error = error
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();

    let (message, pill_state) = match status {
        BackendDictationStatus::Listening => {
            let message = match runtime.mode {
                HotkeyDeliveryMode::GlobalHold | HotkeyDeliveryMode::FocusedWindowHold => {
                    format!("Listening - release {label}")
                }
                HotkeyDeliveryMode::GlobalToggle => format!("Listening - press {label} again"),
                HotkeyDeliveryMode::Disabled => "Listening...".to_string(),
            };
            (message, "live")
        }
        BackendDictationStatus::Processing => ("Transcribing...".to_string(), "working"),
        BackendDictationStatus::Error => (
            if has_error {
                "Dictation error - check status".to_string()
            } else {
                idle_pill_message(app)
            },
            if has_error { "error" } else { "idle" },
        ),
        BackendDictationStatus::Idle => (idle_pill_message(app), "idle"),
    };

    emit_pill_status(
        app,
        message,
        pill_state,
        pill_should_be_visible_for_backend_state(
            status,
            preferences.pill_visibility_mode,
            has_error,
        ),
    );
}

#[cfg(target_os = "macos")]
pub(crate) fn pill_window_width_for_monitor(monitor: &tauri::Monitor) -> f64 {
    let clamped_scale = monitor.scale_factor().clamp(1.0, 2.0);
    PILL_WINDOW_MIN_WIDTH + (clamped_scale - 1.0) * (PILL_WINDOW_BASE_WIDTH - PILL_WINDOW_MIN_WIDTH)
}

#[cfg(target_os = "macos")]
pub(crate) fn create_pill_overlay_window_for_monitor(
    app: &tauri::AppHandle,
    label: &str,
    monitor: &tauri::Monitor,
) -> Result<(), String> {
    if app.get_webview_window(label).is_some() {
        return Ok(());
    }

    let work_area = monitor.work_area();
    let work_x = work_area.position.x;
    let work_y = work_area.position.y;
    let work_w = work_area.size.width as i32;
    let work_h = work_area.size.height as i32;
    let width = pill_window_width_for_monitor(monitor);
    let width_i = width as i32;
    let height_i = PILL_WINDOW_HEIGHT as i32;

    let x = work_x + (work_w - width_i).max(0) / 2;
    let y = work_y + (work_h - height_i - PILL_WINDOW_BOTTOM_MARGIN).max(0);

    let window =
        tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("pill.html".into()))
            .title("dicktaint overlay")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .focusable(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .inner_size(width, PILL_WINDOW_HEIGHT)
            .position(x as f64, y as f64)
            .build()
            .map_err(|e| format!("Failed to create overlay window '{label}': {e}"))?;

    let _ = window.set_ignore_cursor_events(true);
    let _ = window.set_always_on_top(true);
    let _ = window.set_visible_on_all_workspaces(true);
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn create_pill_overlay_windows(app: &tauri::AppHandle) -> Result<(), String> {
    let monitors = app
        .available_monitors()
        .map_err(|e| format!("Failed to enumerate monitors for overlay pill: {e}"))?;
    if monitors.is_empty() {
        return Err("No monitors found while creating overlay pill windows.".to_string());
    }

    for (index, monitor) in monitors.iter().enumerate().take(MAX_PILL_WINDOWS) {
        let label = format!("{PILL_WINDOW_LABEL_PREFIX}-{index}");
        create_pill_overlay_window_for_monitor(app, &label, monitor)?;
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn create_pill_overlay_windows(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}
