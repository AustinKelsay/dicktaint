//! Global hotkeys, pill overlay windows, menu bar tray, and background UI sync.

use crate::commands::{
    cancel_native_dictation_if_active, start_native_dictation_inner, stop_native_dictation_inner,
};
use crate::state::{
    BackendDictationStatus, BackendHotkeyAction, CloseAction,
    DICTATION_STATE_EVENT, DictationTriggerPayload, LocalModelState, LocalSettings,
    MenuBarMode, PILL_STATUS_EVENT, PillStatusPayload, PillVisibilityMode, TrayState,
    DEFAULT_DICTATION_TRIGGER, MAX_DICTATION_TRIGGER_LENGTH, MAIN_TRAY_ID, PILL_WINDOW_BASE_WIDTH,
    PILL_WINDOW_BOTTOM_MARGIN, PILL_WINDOW_HEIGHT, PILL_WINDOW_LABEL_PREFIX, PILL_WINDOW_MIN_WIDTH,
    MAX_PILL_WINDOWS, START_HIDDEN_ENV, TRAY_MENU_FORCE_STOP_ID, TRAY_MENU_OPEN_ID,
    TRAY_MENU_QUIT_ID, TRAY_MENU_STATUS_ID, TRAY_MENU_TOGGLE_ID, BackgroundUiPreferences,
    BackgroundUiPreferencesPayload, DictationState, DictationStatePayload,
    resolve_background_ui_preferences,
};
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, MenuItemBuilder};
#[cfg(target_os = "macos")]
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[cfg(target_os = "macos")]
use crate::state::{
    CFAllocatorRef, CFMachPortRef, CFRunLoopRef, CFRunLoopSourceRef, CFStringRef, CGEventFlags,
    CGEventMask, CGEventRef, CGEventTapProxy, MacFnEventTapCallback, TrayRuntimeState,
    CG_EVENT_TAP_LOCATION_SESSION, CG_EVENT_TAP_PLACEMENT_HEAD_INSERT,
    CG_EVENT_TAP_OPTION_LISTEN_ONLY, CG_EVENT_TYPE_FLAGS_CHANGED,
    CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT, CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT,
    MACOS_FN_FLAG_MASK, MACOS_NON_FN_MODIFIER_MASK,
};

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: CGEventMask,
        callback: MacFnEventTapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetFlags(event: CGEventRef) -> CGEventFlags;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFAllocatorDefault: CFAllocatorRef;
    static kCFRunLoopCommonModes: CFStringRef;

    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFMachPortInvalidate(port: CFMachPortRef);
    fn CFRunLoopGetMain() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRelease(cf: *const c_void);
}



#[derive(Clone, Default)]
pub(crate) enum HotkeyDeliveryMode {
    #[default]
    Disabled,
    GlobalToggle,
    GlobalHold,
    FocusedWindowHold,
}

impl HotkeyDeliveryMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::GlobalToggle => "global-toggle",
            Self::GlobalHold => "global-hold",
            Self::FocusedWindowHold => "focused-window-hold",
        }
    }
}

#[derive(Clone)]
pub(crate) struct TriggerRuntimeDetails {
    pub(crate) mode: HotkeyDeliveryMode,
    pub(crate) status: String,
    pub(crate) permission_hint: Option<String>,
}

#[cfg(target_os = "macos")]
pub(crate) struct MacFnCallbackContext {
    app: tauri::AppHandle,
    enabled: AtomicBool,
    fn_down: AtomicBool,
    tap: AtomicPtr<c_void>,
}

#[cfg(target_os = "macos")]
pub(crate) struct MacFnGlobalListener {
    tap: CFMachPortRef,
    source: CFRunLoopSourceRef,
    callback_ctx: Arc<MacFnCallbackContext>,
    callback_ctx_raw: *const MacFnCallbackContext,
}

#[derive(Default)]
pub(crate) struct GlobalHotkeyState {
    pub(crate) registered_trigger: Mutex<Option<String>>,
    pub(crate) runtime_details: Mutex<TriggerRuntimeDetails>,
    #[cfg(target_os = "macos")]
    pub(crate) macos_fn_listener: Mutex<Option<MacFnGlobalListener>>,
}

#[cfg(target_os = "macos")]
unsafe impl Send for MacFnGlobalListener {}
#[cfg(target_os = "macos")]
unsafe impl Sync for MacFnGlobalListener {}

#[cfg(target_os = "macos")]
pub(crate) fn macos_listener_disable_should_dispatch_stop(was_fn_down: bool) -> bool {
    was_fn_down
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_tap_disable_should_dispatch_stop(was_fn_down: bool) -> bool {
    was_fn_down
}

#[cfg(target_os = "macos")]
impl MacFnGlobalListener {
    pub(crate) fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let callback_ctx = Arc::new(MacFnCallbackContext {
            app: app.clone(),
            enabled: AtomicBool::new(false),
            fn_down: AtomicBool::new(false),
            tap: AtomicPtr::new(std::ptr::null_mut()),
        });
        let callback_ctx_raw = Arc::into_raw(Arc::clone(&callback_ctx));

        let event_mask = 1_u64 << CG_EVENT_TYPE_FLAGS_CHANGED;
        let tap = unsafe {
            CGEventTapCreate(
                CG_EVENT_TAP_LOCATION_SESSION,
                CG_EVENT_TAP_PLACEMENT_HEAD_INSERT,
                CG_EVENT_TAP_OPTION_LISTEN_ONLY,
                event_mask,
                macos_fn_event_tap_callback,
                callback_ctx_raw as *mut c_void,
            )
        };
        if tap.is_null() {
            unsafe {
                drop(Arc::from_raw(callback_ctx_raw));
            }
            return Err("Global Fn listener unavailable. macOS may be blocking event taps. Allow Input Monitoring for this app/terminal in System Settings > Privacy & Security > Input Monitoring.".to_string());
        }

        callback_ctx
            .tap
            .store(tap.cast::<c_void>(), Ordering::SeqCst);

        let source = unsafe { CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) };
        if source.is_null() {
            unsafe {
                CFMachPortInvalidate(tap);
                CFRelease(tap as *const c_void);
                drop(Arc::from_raw(callback_ctx_raw));
            }
            return Err(
                "Failed to create macOS run loop source for global Fn listener.".to_string(),
            );
        }

        unsafe {
            let run_loop = CFRunLoopGetMain();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
        }

        Ok(Self {
            tap,
            source,
            callback_ctx,
            callback_ctx_raw,
        })
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.callback_ctx.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            let was_fn_down = self.callback_ctx.fn_down.swap(false, Ordering::SeqCst);
            if macos_listener_disable_should_dispatch_stop(was_fn_down) {
                dispatch_backend_hotkey_action(
                    &self.callback_ctx.app,
                    BackendHotkeyAction::HoldStop,
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacFnGlobalListener {
    fn drop(&mut self) {
        unsafe {
            let run_loop = CFRunLoopGetMain();
            CFRunLoopRemoveSource(run_loop, self.source, kCFRunLoopCommonModes);
            CFRelease(self.source as *const c_void);

            CFMachPortInvalidate(self.tap);
            CFRelease(self.tap as *const c_void);

            drop(Arc::from_raw(self.callback_ctx_raw));
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_fn_event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if user_info.is_null() {
        return event;
    }

    let callback_ctx = &*(user_info as *const MacFnCallbackContext);
    if event_type == CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT
        || event_type == CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT
    {
        let was_fn_down = callback_ctx.fn_down.swap(false, Ordering::Relaxed);
        if macos_tap_disable_should_dispatch_stop(was_fn_down) {
            dispatch_backend_hotkey_action(&callback_ctx.app, BackendHotkeyAction::HoldStop);
        }
        let tap = callback_ctx.tap.load(Ordering::Relaxed);
        if !tap.is_null() {
            CGEventTapEnable(tap.cast::<c_void>(), true);
        }
        return event;
    }

    if event.is_null() || event_type != CG_EVENT_TYPE_FLAGS_CHANGED {
        return event;
    }

    if !callback_ctx.enabled.load(Ordering::Relaxed) {
        return event;
    }

    let flags = CGEventGetFlags(event);
    let fn_down = (flags & MACOS_FN_FLAG_MASK) != 0;
    let was_fn_down = callback_ctx.fn_down.swap(fn_down, Ordering::Relaxed);

    let has_non_fn_modifiers = (flags & MACOS_NON_FN_MODIFIER_MASK) != 0;
    if fn_down != was_fn_down && !has_non_fn_modifiers {
        dispatch_backend_hotkey_action(
            &callback_ctx.app,
            if fn_down {
                BackendHotkeyAction::HoldStart
            } else {
                BackendHotkeyAction::HoldStop
            },
        );
    }

    event
}
pub(crate) fn parse_truthy_env(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    !matches!(normalized.as_str(), "" | "0" | "false" | "no" | "off")
}

pub(crate) fn should_start_hidden() -> bool {
    std::env::var(START_HIDDEN_ENV)
        .map(|value| parse_truthy_env(&value))
        .unwrap_or(false)
}

pub(crate) fn background_ui_preferences_payload(
    preferences: BackgroundUiPreferences,
) -> BackgroundUiPreferencesPayload {
    BackgroundUiPreferencesPayload {
        pill_visibility_mode: preferences.pill_visibility_mode.as_str().to_string(),
        menu_bar_mode: preferences.menu_bar_mode.as_str().to_string(),
        close_action: preferences.close_action.as_str().to_string(),
    }
}

pub(crate) fn current_background_ui_preferences(
    app: &tauri::AppHandle,
) -> Result<BackgroundUiPreferences, String> {
    let settings = app
        .state::<LocalModelState>()
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    Ok(resolve_background_ui_preferences(&settings))
}

pub(crate) fn current_backend_dictation_status(
    app: &tauri::AppHandle,
) -> Result<BackendDictationStatus, String> {
    app.state::<DictationState>()
        .backend_status
        .lock()
        .map_err(|_| "Failed to lock backend dictation status".to_string())
        .map(|guard| *guard)
}

pub(crate) fn current_backend_error_message(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    app.state::<DictationState>()
        .last_error_message
        .lock()
        .map_err(|_| "Failed to lock backend dictation error state".to_string())
        .map(|guard| guard.clone())
}

pub(crate) fn set_backend_dictation_status(
    app: &tauri::AppHandle,
    status: BackendDictationStatus,
    error: Option<String>,
) -> Result<(), String> {
    let dictation = app.state::<DictationState>();
    {
        let mut guard = dictation
            .backend_status
            .lock()
            .map_err(|_| "Failed to lock backend dictation status".to_string())?;
        *guard = status;
    }
    let mut error_guard = dictation
        .last_error_message
        .lock()
        .map_err(|_| "Failed to lock backend dictation error state".to_string())?;
    *error_guard = if status == BackendDictationStatus::Error {
        error.filter(|value| !value.trim().is_empty())
    } else {
        None
    };
    Ok(())
}

pub(crate) fn main_window_is_visible(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

pub(crate) fn should_show_tray_icon(preferences: BackgroundUiPreferences, main_window_visible: bool) -> bool {
    match preferences.menu_bar_mode {
        MenuBarMode::Always => true,
        MenuBarMode::BackgroundOnly => !main_window_visible,
        MenuBarMode::Off => false,
    }
}

pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    sync_background_ui(app);
}

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
pub(crate) fn tray_primary_action_label(status: BackendDictationStatus) -> &'static str {
    match status {
        BackendDictationStatus::Listening => "Stop + Transcribe",
        BackendDictationStatus::Processing => "Transcribing...",
        BackendDictationStatus::Idle | BackendDictationStatus::Error => "Start Dictation",
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn tray_primary_action_enabled(status: BackendDictationStatus) -> bool {
    status != BackendDictationStatus::Processing
}

#[cfg(target_os = "macos")]
pub(crate) fn tray_force_stop_enabled(status: BackendDictationStatus) -> bool {
    status == BackendDictationStatus::Listening
}

#[cfg(target_os = "macos")]
pub(crate) fn tray_title_for_backend_status(status: BackendDictationStatus) -> &'static str {
    match status {
        BackendDictationStatus::Idle => "DT",
        BackendDictationStatus::Listening => "REC",
        BackendDictationStatus::Processing => "...",
        BackendDictationStatus::Error => "ERR",
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn destroy_macos_tray_runtime(app: &tauri::AppHandle) -> Result<(), String> {
    let tray_state = app.state::<TrayState>();
    let mut guard = tray_state
        .runtime
        .lock()
        .map_err(|_| "Failed to lock tray runtime state".to_string())?;
    *guard = None;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn handle_tray_menu_event(app: &tauri::AppHandle, menu_id: &tauri::menu::MenuId) {
    if menu_id == TRAY_MENU_STATUS_ID {
        return;
    }

    if menu_id == TRAY_MENU_OPEN_ID {
        show_main_window(app);
        return;
    }

    if menu_id == TRAY_MENU_FORCE_STOP_ID {
        if let Err(error) = cancel_native_dictation_if_active(app) {
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
        if let Err(error) = cancel_native_dictation_if_active(app) {
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
            Ok(BackendDictationStatus::Listening) => stop_native_dictation_inner(handle.clone())
                .await
                .map(|_| ()),
            Ok(BackendDictationStatus::Processing) => Ok(()),
            Ok(BackendDictationStatus::Idle | BackendDictationStatus::Error) => {
                start_native_dictation_inner(&handle).map(|_| ())
            }
            Err(error) => Err(error),
        };

        if let Err(error) = result {
            let trimmed = error.trim();
            let benign =
                trimmed == "Dictation already running." || trimmed == "Dictation is not running.";
            if !benign {
                log::warn!("Tray dictation action failed: {error}");
                emit_dictation_state(&handle, "error", Some(error), None, None);
            }
        }
    });
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

pub(crate) fn sync_background_ui(app: &tauri::AppHandle) {
    let status = current_backend_dictation_status(app).unwrap_or_default();
    let error = current_backend_error_message(app).ok().flatten();
    sync_pill_for_backend_state(app, status, error.as_deref());
    #[cfg(target_os = "macos")]
    if let Err(error) = sync_macos_tray(app, status) {
        log::warn!("Failed to sync macOS tray state: {error}");
    }
}

pub(crate) fn emit_dictation_state(
    app: &tauri::AppHandle,
    state: &str,
    error: Option<String>,
    transcript: Option<String>,
    session_id: Option<u64>,
) {
    let backend_status = match state {
        "listening" => BackendDictationStatus::Listening,
        "processing" => BackendDictationStatus::Processing,
        "error" => BackendDictationStatus::Error,
        _ => BackendDictationStatus::Idle,
    };
    if let Err(status_error) = set_backend_dictation_status(app, backend_status, error.clone()) {
        log::warn!("Failed to update backend dictation status: {status_error}");
    }
    sync_background_ui(app);
    app.emit(
        DICTATION_STATE_EVENT,
        DictationStatePayload {
            state: state.to_string(),
            error,
            transcript,
            session_id,
        },
    )
    .ok();
}

pub(crate) fn current_active_session_id(app: &tauri::AppHandle) -> Result<Option<u64>, String> {
    let dictation = app.state::<DictationState>();
    dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())
        .map(|guard| guard.as_ref().map(|recording| recording.session_id))
}

pub(crate) fn dictation_is_running(app: &tauri::AppHandle) -> Result<bool, String> {
    current_active_session_id(app).map(|value| value.is_some())
}

pub(crate) fn dispatch_backend_hotkey_action(app: &tauri::AppHandle, action: BackendHotkeyAction) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result: Result<(), String> = match action {
            BackendHotkeyAction::Toggle => match dictation_is_running(&handle) {
                Ok(true) => stop_native_dictation_inner(handle.clone())
                    .await
                    .map(|_| ()),
                Ok(false) => start_native_dictation_inner(&handle).map(|_| ()),
                Err(error) => Err(error),
            },
            BackendHotkeyAction::HoldStart => match dictation_is_running(&handle) {
                Ok(true) => Ok(()),
                Ok(false) => start_native_dictation_inner(&handle).map(|_| ()),
                Err(error) => Err(error),
            },
            BackendHotkeyAction::HoldStop => match dictation_is_running(&handle) {
                Ok(true) => stop_native_dictation_inner(handle.clone())
                    .await
                    .map(|_| ()),
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            },
        };

        if let Err(error) = result {
            let trimmed = error.trim();
            let benign =
                trimmed == "Dictation already running." || trimmed == "Dictation is not running.";
            if !benign {
                log::warn!("Global hotkey action failed: {error}");
                emit_dictation_state(&handle, "error", Some(error), None, None);
            }
        }
    });
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
pub(crate) fn canonicalize_trigger_modifier(token: &str) -> Option<&'static str> {
    match token.to_ascii_lowercase().as_str() {
        "cmdorctrl" | "commandorcontrol" | "mod" | "primary" => Some("CmdOrCtrl"),
        "cmd" | "command" => Some("Cmd"),
        "ctrl" | "control" => Some("Ctrl"),
        "alt" | "option" => Some("Alt"),
        "shift" => Some("Shift"),
        "super" | "meta" | "win" | "windows" => Some("Super"),
        _ => None,
    }
}

pub(crate) fn canonicalize_trigger_key(token: &str) -> Option<String> {
    let trimmed = token.trim();
    let single_char = {
        let mut chars = trimmed.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) if ch.is_ascii_alphanumeric() => Some(ch.to_ascii_uppercase()),
            _ => None,
        }
    };
    if let Some(ch) = single_char {
        return Some(ch.to_string());
    }

    let lower = trimmed.to_ascii_lowercase();
    let special = match lower.as_str() {
        "fn" | "function" | "globe" => Some("Fn"),
        "space" => Some("Space"),
        "tab" => Some("Tab"),
        "enter" | "return" => Some("Enter"),
        "escape" | "esc" => Some("Escape"),
        "backspace" => Some("Backspace"),
        "delete" | "del" => Some("Delete"),
        "up" | "arrowup" => Some("Up"),
        "down" | "arrowdown" => Some("Down"),
        "left" | "arrowleft" => Some("Left"),
        "right" | "arrowright" => Some("Right"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "pageup" => Some("PageUp"),
        "pagedown" => Some("PageDown"),
        "insert" => Some("Insert"),
        _ => None,
    };
    if let Some(name) = special {
        return Some(name.to_string());
    }

    if lower.starts_with('f') {
        let function_num = lower
            .strip_prefix('f')
            .and_then(|num| num.parse::<u8>().ok())?;
        if (1..=24).contains(&function_num) {
            return Some(format!("F{function_num}"));
        }
    }

    None
}

pub(crate) fn normalize_dictation_trigger(trigger: &str) -> Result<String, String> {
    let trimmed = trigger.trim();
    if trimmed.is_empty() {
        return Err("Dictation trigger cannot be empty.".to_string());
    }
    if trimmed.len() > MAX_DICTATION_TRIGGER_LENGTH {
        return Err(format!(
            "Dictation trigger is too long (max {MAX_DICTATION_TRIGGER_LENGTH} characters)."
        ));
    }

    let mut modifiers = HashSet::<String>::new();
    let mut key: Option<String> = None;
    for token in trimmed.split('+').map(str::trim) {
        if token.is_empty() {
            return Err("Dictation trigger contains an empty token.".to_string());
        }

        if let Some(modifier) = canonicalize_trigger_modifier(token) {
            if key.is_some() {
                return Err("Modifier keys must come before the main trigger key.".to_string());
            }
            modifiers.insert(modifier.to_string());
            continue;
        }

        if key.is_some() {
            return Err("Dictation trigger can only contain one main key.".to_string());
        }
        key = Some(
            canonicalize_trigger_key(token).ok_or_else(|| {
                format!(
                    "Unsupported trigger key '{token}'. Use Fn (macOS), letters/numbers, F1-F24, arrows, or common navigation keys."
                )
            })?,
        );
    }

    let key = key.ok_or_else(|| "Dictation trigger is missing its main key.".to_string())?;
    if key == "Fn" {
        if !modifiers.is_empty() {
            return Err("Fn trigger must be used by itself.".to_string());
        }
        return Ok("Fn".to_string());
    }

    if modifiers.is_empty() {
        return Err("Dictation trigger must include at least one modifier key (or use Fn by itself on macOS).".to_string());
    }
    if modifiers.contains("CmdOrCtrl") && (modifiers.contains("Cmd") || modifiers.contains("Ctrl"))
    {
        return Err("Use CmdOrCtrl by itself, or use Cmd/Ctrl explicitly.".to_string());
    }

    let order = ["CmdOrCtrl", "Cmd", "Ctrl", "Alt", "Shift", "Super"];
    let mut parts: Vec<String> = order
        .iter()
        .filter(|name| modifiers.contains(**name))
        .map(|name| (*name).to_string())
        .collect();
    parts.push(key);
    Ok(parts.join("+"))
}

pub(crate) fn default_dictation_trigger() -> String {
    normalize_dictation_trigger(DEFAULT_DICTATION_TRIGGER)
        .unwrap_or_else(|_| DEFAULT_DICTATION_TRIGGER.to_string())
}

pub(crate) fn resolve_effective_dictation_trigger(settings: &LocalSettings) -> Option<String> {
    if let Some(configured) = settings
        .dictation_trigger
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        match normalize_dictation_trigger(configured) {
            Ok(normalized) => return Some(normalized),
            Err(error) => {
                log::warn!("Ignoring invalid persisted dictation trigger '{configured}': {error}");
            }
        }
    }

    if matches!(settings.dictation_trigger_enabled, Some(false)) {
        return None;
    }

    Some(default_dictation_trigger())
}

pub(crate) fn focused_field_insert_enabled(settings: &LocalSettings) -> bool {
    matches!(settings.focused_field_insert_enabled, Some(true))
}

impl Default for TriggerRuntimeDetails {
    fn default() -> Self {
        Self {
            mode: HotkeyDeliveryMode::Disabled,
            status: "Hotkey disabled.".to_string(),
            permission_hint: None,
        }
    }
}

pub(crate) fn global_toggle_status(trigger: &str) -> String {
    format!("Press {trigger} anywhere to start or stop dictation.")
}

pub(crate) fn global_hold_status(trigger: &str) -> String {
    format!("Hold {trigger} anywhere to dictate, then release to transcribe.")
}

pub(crate) fn focused_window_hold_status(trigger: &str) -> String {
    format!(
        "Hold {trigger} to dictate while dicktaint is focused. Grant Input Monitoring for global hold-to-talk."
    )
}

pub(crate) fn fn_permission_hint() -> String {
    "System Settings > Privacy & Security > Input Monitoring: allow dicktaint (or Terminal while running tauri:dev), then relaunch dicktaint.".to_string()
}

pub(crate) fn set_trigger_runtime_details(
    hotkey_state: &GlobalHotkeyState,
    details: TriggerRuntimeDetails,
) -> Result<(), String> {
    let mut guard = hotkey_state
        .runtime_details
        .lock()
        .map_err(|_| "Failed to lock dictation trigger runtime details".to_string())?;
    *guard = details;
    Ok(())
}

pub(crate) fn current_trigger_runtime_details(
    hotkey_state: &GlobalHotkeyState,
) -> Result<TriggerRuntimeDetails, String> {
    hotkey_state
        .runtime_details
        .lock()
        .map_err(|_| "Failed to lock dictation trigger runtime details".to_string())
        .map(|guard| guard.clone())
}

pub(crate) fn runtime_details_for_trigger(
    trigger: Option<&str>,
    mode: HotkeyDeliveryMode,
) -> TriggerRuntimeDetails {
    let normalized = trigger.map(str::trim).filter(|value| !value.is_empty());
    match (normalized, mode) {
        (Some(value), HotkeyDeliveryMode::GlobalToggle) => TriggerRuntimeDetails {
            mode: HotkeyDeliveryMode::GlobalToggle,
            status: global_toggle_status(value),
            permission_hint: None,
        },
        (Some(value), HotkeyDeliveryMode::GlobalHold) => TriggerRuntimeDetails {
            mode: HotkeyDeliveryMode::GlobalHold,
            status: global_hold_status(value),
            permission_hint: None,
        },
        (Some(value), HotkeyDeliveryMode::FocusedWindowHold) => TriggerRuntimeDetails {
            mode: HotkeyDeliveryMode::FocusedWindowHold,
            status: focused_window_hold_status(value),
            permission_hint: Some(fn_permission_hint()),
        },
        _ => TriggerRuntimeDetails::default(),
    }
}

pub(crate) fn onboarding_runtime_details(
    trigger: Option<&str>,
    registered_trigger: Option<&str>,
    registered_runtime: Option<&TriggerRuntimeDetails>,
) -> TriggerRuntimeDetails {
    let normalized = trigger.map(str::trim).filter(|value| !value.is_empty());
    if normalized.is_none() {
        return TriggerRuntimeDetails::default();
    }

    if normalized == registered_trigger {
        if let Some(runtime) = registered_runtime {
            return runtime.clone();
        }
    }

    #[cfg(target_os = "macos")]
    let mode = if normalized == Some("Fn") {
        HotkeyDeliveryMode::FocusedWindowHold
    } else {
        HotkeyDeliveryMode::GlobalToggle
    };

    #[cfg(not(target_os = "macos"))]
    let mode = HotkeyDeliveryMode::GlobalToggle;

    runtime_details_for_trigger(normalized, mode)
}

pub(crate) fn dictation_trigger_payload(
    settings: &LocalSettings,
    runtime: TriggerRuntimeDetails,
) -> DictationTriggerPayload {
    DictationTriggerPayload {
        trigger: resolve_effective_dictation_trigger(settings),
        default_trigger: default_dictation_trigger(),
        trigger_mode: runtime.mode.as_str().to_string(),
        trigger_status: runtime.status,
        trigger_permission_hint: runtime.permission_hint,
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) fn shortcut_from_dictation_trigger(trigger: &str) -> Result<Shortcut, String> {
    let normalized = normalize_dictation_trigger(trigger)?;
    let accelerator = normalized
        .replace("CmdOrCtrl", "CommandOrControl")
        .replace("Cmd", "Command");
    Shortcut::from_str(&accelerator)
        .map_err(|e| format!("Could not parse hotkey '{normalized}' for global registration: {e}"))
}

pub(crate) fn set_registered_hotkey_state(
    hotkey_state: &GlobalHotkeyState,
    next: Option<String>,
) -> Result<(), String> {
    let mut guard = hotkey_state
        .registered_trigger
        .lock()
        .map_err(|_| "Failed to lock global hotkey state".to_string())?;
    *guard = next;
    Ok(())
}

pub(crate) fn update_hotkey_state(
    hotkey_state: &GlobalHotkeyState,
    trigger: Option<String>,
    runtime: TriggerRuntimeDetails,
) -> Result<(), String> {
    set_registered_hotkey_state(hotkey_state, trigger)?;
    set_trigger_runtime_details(hotkey_state, runtime)
}

pub(crate) fn current_registered_hotkey(hotkey_state: &GlobalHotkeyState) -> Result<Option<String>, String> {
    hotkey_state
        .registered_trigger
        .lock()
        .map_err(|_| "Failed to lock global hotkey state".to_string())
        .map(|guard| guard.clone())
}

#[cfg(target_os = "macos")]
pub(crate) fn should_register_global_hotkey(trigger: &str) -> bool {
    trigger != "Fn"
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn should_register_global_hotkey(_trigger: &str) -> bool {
    true
}

#[cfg(target_os = "macos")]
pub(crate) fn set_macos_fn_listener_enabled(
    app: &tauri::AppHandle,
    hotkey_state: &GlobalHotkeyState,
    enabled: bool,
) -> Result<(), String> {
    let mut guard = hotkey_state
        .macos_fn_listener
        .lock()
        .map_err(|_| "Failed to lock macOS Fn listener state".to_string())?;

    if enabled {
        if guard.is_none() {
            *guard = Some(MacFnGlobalListener::new(app)?);
        }
    }

    if let Some(listener) = guard.as_ref() {
        listener.set_enabled(enabled);
    }

    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) fn apply_registered_hotkey(
    app: &tauri::AppHandle,
    hotkey_state: &GlobalHotkeyState,
    trigger: Option<&str>,
) -> Result<TriggerRuntimeDetails, String> {
    let next = match trigger.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Some(normalize_dictation_trigger(value)?),
        None => None,
    };
    let previous = current_registered_hotkey(hotkey_state)?;

    if previous == next && next.as_deref() != Some("Fn") {
        return current_trigger_runtime_details(hotkey_state);
    }

    #[cfg(target_os = "macos")]
    if previous.as_deref() == Some("Fn") {
        if let Err(error) = set_macos_fn_listener_enabled(app, hotkey_state, false) {
            log::warn!("Failed to disable global Fn listener: {error}");
        }
    }

    if let Some(previous_trigger) = previous.as_deref() {
        if should_register_global_hotkey(previous_trigger) {
            let previous_shortcut = shortcut_from_dictation_trigger(previous_trigger)?;
            app.global_shortcut()
                .unregister(previous_shortcut)
                .map_err(|e| {
                    format!("Failed to unregister previous global hotkey '{previous_trigger}': {e}")
                })?;
        }
    }

    if let Some(next_trigger) = next.as_deref() {
        if should_register_global_hotkey(next_trigger) {
            let next_shortcut = shortcut_from_dictation_trigger(next_trigger)?;
            if let Err(error) = app.global_shortcut().register(next_shortcut) {
                if let Some(previous_trigger) = previous.as_deref() {
                    if should_register_global_hotkey(previous_trigger) {
                        if let Ok(previous_shortcut) =
                            shortcut_from_dictation_trigger(previous_trigger)
                        {
                            if let Err(recovery_error) =
                                app.global_shortcut().register(previous_shortcut)
                            {
                                update_hotkey_state(
                                    hotkey_state,
                                    None,
                                    TriggerRuntimeDetails::default(),
                                )?;
                                return Err(format!(
                                    "Could not register global hotkey '{next_trigger}': {error}. Also failed to restore previous hotkey '{previous_trigger}': {recovery_error}"
                                ));
                            }
                            update_hotkey_state(
                                hotkey_state,
                                Some(previous_trigger.to_string()),
                                runtime_details_for_trigger(
                                    Some(previous_trigger),
                                    HotkeyDeliveryMode::GlobalToggle,
                                ),
                            )?;
                            #[cfg(target_os = "macos")]
                            if previous_trigger == "Fn" {
                                if let Err(listener_error) =
                                    set_macos_fn_listener_enabled(app, hotkey_state, true)
                                {
                                    update_hotkey_state(
                                        hotkey_state,
                                        Some(previous_trigger.to_string()),
                                        runtime_details_for_trigger(
                                            Some(previous_trigger),
                                            HotkeyDeliveryMode::FocusedWindowHold,
                                        ),
                                    )?;
                                    log::warn!(
                                        "Failed to re-enable global Fn listener after hotkey restore: {listener_error}"
                                    );
                                }
                            }
                        } else {
                            update_hotkey_state(
                                hotkey_state,
                                None,
                                TriggerRuntimeDetails::default(),
                            )?;
                        }
                    } else {
                        #[cfg(target_os = "macos")]
                        let restored_runtime = if previous_trigger == "Fn" {
                            match set_macos_fn_listener_enabled(app, hotkey_state, true) {
                                Ok(()) => runtime_details_for_trigger(
                                    Some(previous_trigger),
                                    HotkeyDeliveryMode::GlobalHold,
                                ),
                                Err(listener_error) => {
                                    log::warn!(
                                        "Failed to re-enable global Fn listener after hotkey restore: {listener_error}"
                                    );
                                    runtime_details_for_trigger(
                                        Some(previous_trigger),
                                        HotkeyDeliveryMode::FocusedWindowHold,
                                    )
                                }
                            }
                        } else {
                            runtime_details_for_trigger(
                                Some(previous_trigger),
                                HotkeyDeliveryMode::GlobalToggle,
                            )
                        };

                        #[cfg(not(target_os = "macos"))]
                        let restored_runtime = runtime_details_for_trigger(
                            Some(previous_trigger),
                            HotkeyDeliveryMode::GlobalToggle,
                        );

                        update_hotkey_state(
                            hotkey_state,
                            Some(previous_trigger.to_string()),
                            restored_runtime,
                        )?;
                    }
                } else {
                    update_hotkey_state(hotkey_state, None, TriggerRuntimeDetails::default())?;
                }
                return Err(format!(
                    "Could not register global hotkey '{next_trigger}': {error}"
                ));
            }
        }
    }

    #[cfg(target_os = "macos")]
    let runtime = if let Some(next_trigger) = next.as_deref() {
        if next_trigger == "Fn" {
            match set_macos_fn_listener_enabled(app, hotkey_state, true) {
                Ok(()) => {
                    runtime_details_for_trigger(Some(next_trigger), HotkeyDeliveryMode::GlobalHold)
                }
                Err(error) => {
                    log::warn!(
                        "Global Fn listener unavailable; falling back to in-app Fn hotkey handling: {error}"
                    );
                    runtime_details_for_trigger(
                        Some(next_trigger),
                        HotkeyDeliveryMode::FocusedWindowHold,
                    )
                }
            }
        } else {
            runtime_details_for_trigger(Some(next_trigger), HotkeyDeliveryMode::GlobalToggle)
        }
    } else {
        TriggerRuntimeDetails::default()
    };

    #[cfg(not(target_os = "macos"))]
    let runtime = if let Some(next_trigger) = next.as_deref() {
        runtime_details_for_trigger(Some(next_trigger), HotkeyDeliveryMode::GlobalToggle)
    } else {
        TriggerRuntimeDetails::default()
    };

    update_hotkey_state(hotkey_state, next, runtime.clone())?;
    Ok(runtime)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) fn apply_registered_hotkey(
    _app: &tauri::AppHandle,
    hotkey_state: &GlobalHotkeyState,
    trigger: Option<&str>,
) -> Result<TriggerRuntimeDetails, String> {
    let next = match trigger.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Some(normalize_dictation_trigger(value)?),
        None => None,
    };
    let runtime = if let Some(next_trigger) = next.as_deref() {
        runtime_details_for_trigger(Some(next_trigger), HotkeyDeliveryMode::GlobalToggle)
    } else {
        TriggerRuntimeDetails::default()
    };
    update_hotkey_state(hotkey_state, next, runtime.clone())?;
    Ok(runtime)
}
