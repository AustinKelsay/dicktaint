//! macOS Fn/Globe global listener via CGEventTap FFI.
//!
//! Prefers a HID-level listen-only tap (more reliable for background Fn/Globe),
//! falls back to a session tap, and treats Input Monitoring preflight as a soft
//! prompt rather than a hard gate — preflight can report true while hardware
//! events still never arrive after ad-hoc re-signing.

use crate::state::{BackendHotkeyAction, CFAllocatorRef, CFMachPortRef, CFRunLoopRef,
    CFRunLoopSourceRef, CFStringRef, CGEventFlags, CGEventMask, CGEventRef, CGEventTapProxy,
    MacFnEventTapCallback, CG_EVENT_TAP_LOCATION_HID, CG_EVENT_TAP_LOCATION_SESSION,
    CG_EVENT_TAP_OPTION_LISTEN_ONLY, CG_EVENT_TAP_PLACEMENT_HEAD_INSERT,
    CG_EVENT_TYPE_FLAGS_CHANGED, CG_EVENT_TYPE_KEY_DOWN, CG_EVENT_TYPE_KEY_UP,
    CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT, CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT,
    CG_KEYBOARD_EVENT_KEYCODE, KEYCODE_FN, MACOS_FN_FLAG_MASK, MACOS_NON_FN_MODIFIER_MASK};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use super::dispatch_backend_hotkey_action;

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
    /// Returns whether this process may create listen-only event taps.
    ///
    /// Not a hard gate: on some macOS builds this reports true while hardware
    /// Fn events still never reach a listen-only tap after identity changes.
    fn CGPreflightListenEventAccess() -> bool;
    /// Prompts the user (and opens Input Monitoring settings) when access is missing.
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

struct MacFnCallbackContext {
    app: tauri::AppHandle,
    enabled: AtomicBool,
    fn_down: AtomicBool,
    tap: AtomicPtr<c_void>,
}

pub(super) struct MacFnGlobalListener {
    tap: CFMachPortRef,
    source: CFRunLoopSourceRef,
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

/// Error shown when Input Monitoring (or tap creation) blocks global Fn hold.
fn macos_fn_input_monitoring_error() -> String {
    "Global Fn listener unavailable. macOS may be blocking event taps. Allow Input Monitoring for this app/terminal in System Settings > Privacy & Security > Input Monitoring.".to_string()
}

/// Soft-prompts for Input Monitoring when preflight says access is missing.
fn prompt_macos_listen_event_access_if_needed() {
    unsafe {
        if !CGPreflightListenEventAccess() {
            let _ = CGRequestListenEventAccess();
        }
    }
}

/// Event mask for Fn/Globe: modifier flag changes plus Fn keycode down/up.
fn macos_fn_event_mask() -> CGEventMask {
    (1_u64 << CG_EVENT_TYPE_FLAGS_CHANGED)
        | (1_u64 << CG_EVENT_TYPE_KEY_DOWN)
        | (1_u64 << CG_EVENT_TYPE_KEY_UP)
}

/**
 * Creates a listen-only tap, preferring HID then session.
 *
 * HID sees hardware Fn/Globe more reliably for background hold-to-talk when
 * Input Monitoring / Accessibility allow it.
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

/// Resolves Fn/Globe pressed state from a flagsChanged or Fn keycode event.
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

        let mut tap = create_macos_fn_event_tap(callback_ctx_raw as *mut c_void);
        if tap.is_null() {
            // One more prompt + retry — first launch often needs the Settings toggle.
            unsafe {
                let _ = CGRequestListenEventAccess();
            }
            tap = create_macos_fn_event_tap(callback_ctx_raw as *mut c_void);
        }
        if tap.is_null() {
            unsafe {
                drop(Arc::from_raw(callback_ctx_raw));
            }
            return Err(macos_fn_input_monitoring_error());
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

    if !callback_ctx.enabled.load(Ordering::Relaxed) {
        return event;
    }

    let Some(fn_down) = fn_down_from_event(event_type, event) else {
        return event;
    };

    let was_fn_down = callback_ctx.fn_down.swap(fn_down, Ordering::Relaxed);
    if fn_down != was_fn_down {
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
        assert!(message.contains("Input Monitoring"));
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
        assert_eq!(fn_down_from_event(CG_EVENT_TYPE_FLAGS_CHANGED, std::ptr::null()), None);
    }
}
