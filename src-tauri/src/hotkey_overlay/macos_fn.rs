//! macOS Fn/Globe global listener via CGEventTap FFI.

use crate::state::{BackendHotkeyAction, CFAllocatorRef, CFMachPortRef, CFRunLoopRef,
    CFRunLoopSourceRef, CFStringRef, CGEventFlags, CGEventMask, CGEventRef, CGEventTapProxy,
    MacFnEventTapCallback, CG_EVENT_TAP_LOCATION_SESSION, CG_EVENT_TAP_OPTION_LISTEN_ONLY,
    CG_EVENT_TAP_PLACEMENT_HEAD_INSERT, CG_EVENT_TYPE_FLAGS_CHANGED,
    CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT, CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT,
    MACOS_FN_FLAG_MASK, MACOS_NON_FN_MODIFIER_MASK};
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

impl MacFnGlobalListener {
    pub(super) fn new(app: &tauri::AppHandle) -> Result<Self, String> {
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



#[cfg(test)]
mod tests {
    use super::{
        macos_listener_disable_should_dispatch_stop, macos_tap_disable_should_dispatch_stop,
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
}
