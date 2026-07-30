//! Paste dictated text into the focused field via macOS Accessibility APIs.

use crate::state::{
    FocusedFieldInsertPermissionStatus, KEYCODE_COMMAND, KEYCODE_V, MACOS_COMMAND_FLAG_MASK,
    CG_EVENT_TAP_LOCATION_HID, CGEventFlags, CGEventRef,
};
use std::thread;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
use std::process::Command;

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: *const std::ffi::c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
    fn CGEventPost(tap: u32, event: CGEventRef);
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}



#[cfg(target_os = "macos")]
pub(crate) fn macos_accessibility_permission_granted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn macos_accessibility_permission_granted() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn open_accessibility_settings() -> Result<(), String> {
    let status = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map_err(|e| format!("Failed to open macOS Accessibility settings: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Failed to open macOS Accessibility settings.".to_string())
    }
}

pub(crate) fn focused_field_insert_permission_status(
    enabled: bool,
    prompt_if_missing: bool,
) -> FocusedFieldInsertPermissionStatus {
    #[cfg(target_os = "macos")]
    {
        let granted = macos_accessibility_permission_granted();
        if granted {
            return FocusedFieldInsertPermissionStatus {
                granted: true,
                status: if enabled {
                    "Accessibility permission granted. Finished transcripts can be pasted into the focused field of other apps."
                        .to_string()
                } else {
                    "Accessibility permission granted. Enable focused-field insertion to paste dictated text into other apps."
                        .to_string()
                },
            };
        }

        if prompt_if_missing {
            if let Err(error) = open_accessibility_settings() {
                log::warn!("Failed to open Accessibility settings: {error}");
            }
        }

        return FocusedFieldInsertPermissionStatus {
            granted: false,
            status: "Focused-field insertion needs Accessibility permission. Opened System Settings > Privacy & Security > Accessibility. Allow dicktaint, then retry the paste."
                .to_string(),
        };
    }

    #[allow(unreachable_code)]
    FocusedFieldInsertPermissionStatus {
        granted: false,
        status: if enabled {
            "Focused-field insertion is currently supported on macOS desktop only.".to_string()
        } else {
            "Focused-field insertion is unavailable on this platform.".to_string()
        },
    }
}
pub(crate) fn write_text_to_general_pasteboard(
    text: &str,
) -> Result<(Retained<NSPasteboard>, Option<String>), String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let snapshot =
        unsafe { pasteboard.stringForType(NSPasteboardTypeString) }.map(|value| value.to_string());

    let _ = pasteboard.clearContents();
    let ns_text = NSString::from_str(text);
    if !unsafe { pasteboard.setString_forType(&ns_text, NSPasteboardTypeString) } {
        return Err("Failed to place dictated text on the macOS pasteboard.".to_string());
    }

    Ok((pasteboard, snapshot))
}

#[cfg(target_os = "macos")]
pub(crate) fn restore_general_pasteboard(
    pasteboard: &NSPasteboard,
    snapshot: Option<String>,
) -> Result<(), String> {
    let Some(previous_text) = snapshot else {
        return Ok(());
    };

    let _ = pasteboard.clearContents();
    let ns_text = NSString::from_str(previous_text.as_str());
    if !unsafe { pasteboard.setString_forType(&ns_text, NSPasteboardTypeString) } {
        return Err(
            "Failed to restore the previous macOS pasteboard text after dictation paste."
                .to_string(),
        );
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn post_keyboard_event(keycode: u16, key_down: bool, flags: CGEventFlags) -> Result<(), String> {
    let event = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), keycode, key_down) };
    if event.is_null() {
        return Err(format!(
            "Failed to create macOS keyboard event for keycode {keycode}."
        ));
    }

    unsafe {
        CGEventSetFlags(event, flags);
        CGEventPost(CG_EVENT_TAP_LOCATION_HID, event);
        CFRelease(event as *const c_void);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn post_command_v_paste() -> Result<(), String> {
    post_keyboard_event(KEYCODE_COMMAND, true, MACOS_COMMAND_FLAG_MASK)?;
    post_keyboard_event(KEYCODE_V, true, MACOS_COMMAND_FLAG_MASK)?;
    post_keyboard_event(KEYCODE_V, false, MACOS_COMMAND_FLAG_MASK)?;
    post_keyboard_event(KEYCODE_COMMAND, false, 0)?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn insert_text_into_focused_field_impl(text: &str) -> Result<(), String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let permission = focused_field_insert_permission_status(true, true);
    if !permission.granted {
        return Err(permission.status);
    }

    let (pasteboard, snapshot) = write_text_to_general_pasteboard(trimmed)?;
    let paste_result = post_command_v_paste();
    thread::sleep(Duration::from_millis(80));
    let restore_result = restore_general_pasteboard(&pasteboard, snapshot);

    if let Err(error) = restore_result {
        log::warn!("{error}");
    }

    paste_result.map_err(|error| {
        format!(
            "Focused field insertion failed while sending native paste keystrokes. Allow Accessibility for dicktaint in System Settings > Privacy & Security > Accessibility, then retry. Details: {error}"
        )
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn insert_text_into_focused_field_impl(_text: &str) -> Result<(), String> {
    Err("Focused field insertion is currently supported on macOS desktop only.".to_string())
}
