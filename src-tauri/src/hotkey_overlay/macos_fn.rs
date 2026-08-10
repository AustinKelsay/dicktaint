//! macOS Fn/Globe global listener.
//!
//! Two delivery paths:
//! 1. `NSEvent` global monitor — uses Accessibility (same grant as focused-field paste)
//! 2. `CGEventTap` listen-only — uses Input Monitoring
//!
//! Focused-window Fn can work via WKWebView without either path. Background Fn
//! needs at least one of these. Ad-hoc re-signs often leave CGEventTap silent
//! even when `CGPreflightListenEventAccess` reports true, so the NSEvent path
//! is the reliable background fallback when Accessibility is already granted.

use crate::state::{BackendHotkeyAction, CFAllocatorRef, CFMachPortRef, CFRunLoopRef,
    CFRunLoopSourceRef, CFStringRef, CGEventFlags, CGEventMask, CGEventRef, CGEventTapProxy,
    MacFnEventTapCallback, CG_EVENT_TAP_LOCATION_HID, CG_EVENT_TAP_LOCATION_SESSION,
    CG_EVENT_TAP_OPTION_LISTEN_ONLY, CG_EVENT_TAP_PLACEMENT_HEAD_INSERT,
    CG_EVENT_TYPE_FLAGS_CHANGED, CG_EVENT_TYPE_KEY_DOWN, CG_EVENT_TYPE_KEY_UP,
    CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT, CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT,
    CG_KEYBOARD_EVENT_KEYCODE, KEYCODE_FN, MACOS_FN_FLAG_MASK, MACOS_NON_FN_MODIFIER_MASK};
use block2::RcBlock;
use objc2::runtime::AnyObject;
use objc2::rc::Retained;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};
use std::ffi::c_void;
use std::process::Command;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::dispatch_backend_hotkey_action;

/// Minimum gap between Input Monitoring Settings reopen attempts.
const INPUT_MONITORING_SETTINGS_REOPEN_COOLDOWN: Duration = Duration::from_secs(60);
/// Minimum gap between focus/reopen Fn tap rebuilds.
const FN_LISTENER_ACTIVATION_REFRESH_COOLDOWN: Duration = Duration::from_secs(5);
static LAST_INPUT_MONITORING_SETTINGS_OPEN: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_FN_LISTENER_ACTIVATION_REFRESH: Mutex<Option<Instant>> = Mutex::new(None);
static DID_REQUEST_LISTEN_EVENT_ACCESS: AtomicBool = AtomicBool::new(false);

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
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

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

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

struct MacFnCallbackContext {
    app: tauri::AppHandle,
    enabled: AtomicBool,
    fn_down: AtomicBool,
    tap: AtomicPtr<c_void>,
}

pub(super) struct MacFnGlobalListener {
    tap: Option<CFMachPortRef>,
    source: Option<CFRunLoopSourceRef>,
    nsevent_monitor: Option<Retained<AnyObject>>,
    /// Keeps the NSEvent handler block alive for the monitor lifetime.
    _nsevent_block: Option<RcBlock<dyn Fn(NonNull<NSEvent>)>>,
    callback_ctx: Arc<MacFnCallbackContext>,
    callback_ctx_raw: *const MacFnCallbackContext,
}

unsafe impl Send for MacFnGlobalListener {}
unsafe impl Sync for MacFnGlobalListener {}

fn macos_listener_disable_should_dispatch_stop(was_fn_down: bool) -> bool {
    was_fn_down
}

fn macos_tap_disable_should_dispatch_stop(was_fn_down: bool) -> bool {
    was_fn_down
}

/// Error shown when neither Accessibility nor Input Monitoring can arm global Fn.
fn macos_fn_input_monitoring_error() -> String {
    "Global Fn listener unavailable. Allow Accessibility (and Input Monitoring if listed) for dicktaint in System Settings > Privacy & Security, then relaunch dicktaint.".to_string()
}

/// Opens System Settings → Input Monitoring (rate-limited).
fn open_input_monitoring_settings_debounced() {
    let mut last_open = match LAST_INPUT_MONITORING_SETTINGS_OPEN.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(opened_at) = *last_open {
        if opened_at.elapsed() < INPUT_MONITORING_SETTINGS_REOPEN_COOLDOWN {
            return;
        }
    }

    let status = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        .status();
    match status {
        Ok(code) if code.success() => {
            *last_open = Some(Instant::now());
        }
        Ok(code) => {
            log::warn!("Failed to open Input Monitoring settings (exit {code}).");
        }
        Err(error) => {
            log::warn!("Failed to open Input Monitoring settings: {error}");
        }
    }
}

/**
 * Requests Input Monitoring once per process and opens Settings when denied.
 *
 * The first request matters after ad-hoc re-signs: macOS keys the toggle to the
 * new CDHash, and without a request the app may never appear under Input
 * Monitoring even though `CGEventTapCreate` still returns a non-null (silent) tap.
 */
fn prompt_macos_listen_event_access_if_needed() {
    let already_requested = DID_REQUEST_LISTEN_EVENT_ACCESS.swap(true, Ordering::SeqCst);
    let granted = unsafe {
        if !already_requested || !CGPreflightListenEventAccess() {
            let _ = CGRequestListenEventAccess();
        }
        CGPreflightListenEventAccess()
    };
    if !granted {
        open_input_monitoring_settings_debounced();
    }
}

/// Whether activation should rebuild an existing Fn tap.
pub(crate) fn should_refresh_fn_listener_after_activation(registered_trigger_is_fn: bool) -> bool {
    registered_trigger_is_fn
}

/// Returns whether the activation-refresh cooldown has elapsed.
pub(crate) fn claim_fn_listener_activation_refresh_slot() -> bool {
    let mut last = match LAST_FN_LISTENER_ACTIVATION_REFRESH.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(refreshed_at) = *last {
        if refreshed_at.elapsed() < FN_LISTENER_ACTIVATION_REFRESH_COOLDOWN {
            return false;
        }
    }
    *last = Some(Instant::now());
    true
}

/// Event mask for Fn/Globe: modifier flag changes plus Fn keycode down/up.
fn macos_fn_event_mask() -> CGEventMask {
    (1_u64 << CG_EVENT_TYPE_FLAGS_CHANGED)
        | (1_u64 << CG_EVENT_TYPE_KEY_DOWN)
        | (1_u64 << CG_EVENT_TYPE_KEY_UP)
}

/**
 * Creates a listen-only tap, preferring HID then session.
 */
fn create_macos_fn_event_tap(user_info: *mut c_void) -> CFMachPortRef {
    let event_mask = macos_fn_event_mask();
    unsafe {
        let hid_tap = CGEventTapCreate(
            CG_EVENT_TAP_LOCATION_HID,
            CG_EVENT_TAP_PLACEMENT_HEAD_INSERT,
            CG_EVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            macos_fn_event_tap_callback,
            user_info,
        );
        if !hid_tap.is_null() {
            return hid_tap;
        }

        CGEventTapCreate(
            CG_EVENT_TAP_LOCATION_SESSION,
            CG_EVENT_TAP_PLACEMENT_HEAD_INSERT,
            CG_EVENT_TAP_OPTION_LISTEN_ONLY,
            event_mask,
            macos_fn_event_tap_callback,
            user_info,
        )
    }
}

/// Resolves Fn/Globe pressed state from a CGEvent flagsChanged or Fn keycode event.
fn fn_down_from_event(event_type: u32, event: CGEventRef) -> Option<bool> {
    if event.is_null() {
        return None;
    }

    if event_type == CG_EVENT_TYPE_FLAGS_CHANGED {
        let flags = unsafe { CGEventGetFlags(event) };
        let fn_down = (flags & MACOS_FN_FLAG_MASK) != 0;
        let has_non_fn_modifiers = (flags & MACOS_NON_FN_MODIFIER_MASK) != 0;
        if has_non_fn_modifiers {
            return None;
        }
        return Some(fn_down);
    }

    if event_type == CG_EVENT_TYPE_KEY_DOWN || event_type == CG_EVENT_TYPE_KEY_UP {
        let keycode = unsafe { CGEventGetIntegerValueField(event, CG_KEYBOARD_EVENT_KEYCODE) };
        if keycode != KEYCODE_FN {
            return None;
        }
        return Some(event_type == CG_EVENT_TYPE_KEY_DOWN);
    }

    None
}

/**
 * Resolves Fn pressed state from NSEvent modifier flags / keycode.
 *
 * Uses `NSEventModifierFlagFunction` (Accessibility global monitor path), which
 * is distinct from the CoreGraphics SecondaryFn bit used by CGEventTap.
 */
fn fn_down_from_nsevent(event: &NSEvent) -> Option<bool> {
    let event_type = event.r#type();
    if event_type == NSEventType::FlagsChanged {
        let flags = event.modifierFlags();
        let non_fn = NSEventModifierFlags::Shift
            .union(NSEventModifierFlags::Control)
            .union(NSEventModifierFlags::Option)
            .union(NSEventModifierFlags::Command);
        if flags.intersects(non_fn) {
            return None;
        }
        return Some(flags.contains(NSEventModifierFlags::Function));
    }

    if event_type == NSEventType::KeyDown || event_type == NSEventType::KeyUp {
        if i64::from(event.keyCode()) != KEYCODE_FN {
            return None;
        }
        return Some(event_type == NSEventType::KeyDown);
    }

    None
}

/// Applies a Fn edge transition into HoldStart / HoldStop when enabled.
fn apply_fn_down_state(callback_ctx: &MacFnCallbackContext, fn_down: bool) {
    if !callback_ctx.enabled.load(Ordering::Relaxed) {
        return;
    }

    let was_fn_down = callback_ctx.fn_down.swap(fn_down, Ordering::Relaxed);
    if fn_down == was_fn_down {
        return;
    }

    dispatch_backend_hotkey_action(
        &callback_ctx.app,
        if fn_down {
            BackendHotkeyAction::HoldStart
        } else {
            BackendHotkeyAction::HoldStop
        },
    );
}

/// Installs an Accessibility-backed NSEvent global monitor for Fn/Globe.
fn install_nsevent_fn_monitor(
    callback_ctx: Arc<MacFnCallbackContext>,
) -> (
    Option<Retained<AnyObject>>,
    Option<RcBlock<dyn Fn(NonNull<NSEvent>)>>,
) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!(
            "Accessibility is not trusted; NSEvent global Fn monitor unavailable. Focused-field paste grant also covers background Fn hold."
        );
        return (None, None);
    }

    let block = RcBlock::new(move |event_ptr: NonNull<NSEvent>| {
        let event = unsafe { event_ptr.as_ref() };
        if let Some(fn_down) = fn_down_from_nsevent(event) {
            apply_fn_down_state(&callback_ctx, fn_down);
        }
    });

    let mask = NSEventMask::FlagsChanged
        .union(NSEventMask::KeyDown)
        .union(NSEventMask::KeyUp);
    let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &block);
    if monitor.is_none() {
        log::warn!(
            "NSEvent.addGlobalMonitorForEvents returned nil; grant Accessibility for dicktaint."
        );
    }
    (monitor, Some(block))
}

impl MacFnGlobalListener {
    pub(super) fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        prompt_macos_listen_event_access_if_needed();

        let callback_ctx = Arc::new(MacFnCallbackContext {
            app: app.clone(),
            enabled: AtomicBool::new(false),
            fn_down: AtomicBool::new(false),
            tap: AtomicPtr::new(std::ptr::null_mut()),
        });
        let callback_ctx_raw = Arc::into_raw(Arc::clone(&callback_ctx));

        let (nsevent_monitor, nsevent_block) =
            install_nsevent_fn_monitor(Arc::clone(&callback_ctx));

        let mut tap = create_macos_fn_event_tap(callback_ctx_raw as *mut c_void);
        if tap.is_null() {
            unsafe {
                let _ = CGRequestListenEventAccess();
            }
            tap = create_macos_fn_event_tap(callback_ctx_raw as *mut c_void);
        }

        let mut source = std::ptr::null_mut();
        if !tap.is_null() {
            callback_ctx
                .tap
                .store(tap.cast::<c_void>(), Ordering::SeqCst);
            source = unsafe { CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) };
            if source.is_null() {
                unsafe {
                    CFMachPortInvalidate(tap);
                    CFRelease(tap as *const c_void);
                }
                tap = std::ptr::null_mut();
            } else {
                unsafe {
                    let run_loop = CFRunLoopGetMain();
                    CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
                    CGEventTapEnable(tap, true);
                }
            }
        }

        let has_nsevent = nsevent_monitor.is_some();
        let has_tap = !tap.is_null();
        if !has_nsevent && !has_tap {
            unsafe {
                drop(Arc::from_raw(callback_ctx_raw));
            }
            return Err(macos_fn_input_monitoring_error());
        }

        if has_nsevent && !has_tap {
            log::info!(
                "Global Fn hold armed via Accessibility NSEvent monitor (CGEventTap unavailable)."
            );
        } else if !has_nsevent && has_tap {
            log::info!(
                "Global Fn hold armed via CGEventTap only (Accessibility NSEvent monitor unavailable)."
            );
        } else {
            log::info!("Global Fn hold armed via NSEvent monitor and CGEventTap.");
        }

        Ok(Self {
            tap: if has_tap { Some(tap) } else { None },
            source: if has_tap { Some(source) } else { None },
            nsevent_monitor,
            _nsevent_block: nsevent_block,
            callback_ctx,
            callback_ctx_raw,
        })
    }

    pub(super) fn set_enabled(&self, enabled: bool) {
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

impl Drop for MacFnGlobalListener {
    fn drop(&mut self) {
        if let Some(monitor) = self.nsevent_monitor.take() {
            unsafe {
                NSEvent::removeMonitor(&monitor);
            }
        }

        unsafe {
            if let (Some(source), Some(tap)) = (self.source.take(), self.tap.take()) {
                let run_loop = CFRunLoopGetMain();
                CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
                CFRelease(source as *const c_void);
                CFMachPortInvalidate(tap);
                CFRelease(tap as *const c_void);
            }

            drop(Arc::from_raw(self.callback_ctx_raw));
        }
    }
}

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

    if let Some(fn_down) = fn_down_from_event(event_type, event) {
        apply_fn_down_state(callback_ctx, fn_down);
    }

    event
}

#[cfg(test)]
mod tests {
    use super::{
        fn_down_from_event, macos_fn_event_mask, macos_fn_input_monitoring_error,
        macos_listener_disable_should_dispatch_stop, macos_tap_disable_should_dispatch_stop,
    };
    use crate::state::{
        CG_EVENT_TYPE_FLAGS_CHANGED, CG_EVENT_TYPE_KEY_DOWN, CG_EVENT_TYPE_KEY_UP,
    };

    #[test]
    fn listener_disable_dispatches_stop_when_fn_was_down() {
        assert!(macos_listener_disable_should_dispatch_stop(true));
        assert!(!macos_listener_disable_should_dispatch_stop(false));
    }

    #[test]
    fn tap_disable_dispatches_stop_when_fn_was_down() {
        assert!(macos_tap_disable_should_dispatch_stop(true));
        assert!(!macos_tap_disable_should_dispatch_stop(false));
    }

    #[test]
    fn input_monitoring_error_mentions_settings_path() {
        let message = macos_fn_input_monitoring_error();
        assert!(message.contains("Accessibility") || message.contains("Input Monitoring"));
        assert!(message.contains("Privacy & Security"));
    }

    #[test]
    fn fn_event_mask_includes_flags_and_fn_key_events() {
        let mask = macos_fn_event_mask();
        assert_ne!(mask & (1_u64 << CG_EVENT_TYPE_FLAGS_CHANGED), 0);
        assert_ne!(mask & (1_u64 << CG_EVENT_TYPE_KEY_DOWN), 0);
        assert_ne!(mask & (1_u64 << CG_EVENT_TYPE_KEY_UP), 0);
    }

    #[test]
    fn fn_down_from_event_ignores_null_event() {
        assert_eq!(
            fn_down_from_event(CG_EVENT_TYPE_FLAGS_CHANGED, std::ptr::null()),
            None
        );
    }

    #[test]
    fn activation_refresh_only_when_fn_is_registered() {
        assert!(super::should_refresh_fn_listener_after_activation(true));
        assert!(!super::should_refresh_fn_listener_after_activation(false));
    }
}
