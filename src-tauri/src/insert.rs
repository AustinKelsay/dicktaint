//! Paste dictated text into the focused field via macOS Accessibility APIs.
//!
//! Inserts temporarily own the general pasteboard, post Cmd+V, then restore the
//! prior clipboard. Overlapping inserts used to race that cycle and paste a
//! stale prior dictation — serialization + conditional restore prevent that.

use crate::state::FocusedFieldInsertPermissionStatus;
#[cfg(target_os = "macos")]
use crate::state::{
    KEYCODE_COMMAND, KEYCODE_V, MACOS_COMMAND_FLAG_MASK, CG_EVENT_TAP_LOCATION_HID, CGEventFlags,
    CGEventRef,
};
#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
#[cfg(target_os = "macos")]
use std::process::Command;

/// Process-wide lock so pasteboard write → Cmd+V → restore never overlaps.
#[cfg(target_os = "macos")]
static FOCUSED_FIELD_INSERT_LOCK: Mutex<()> = Mutex::new(());

/// How long to wait after posting Cmd+V before restoring the prior clipboard.
///
/// 80ms was too short for some apps under load; restoring while paste was still
/// reading left the previous dictation on the pasteboard for a late Cmd+V.
#[cfg(target_os = "macos")]
const PASTEBOARD_PASTE_SETTLE: Duration = Duration::from_millis(250);

/// Returns whether restoring the pre-insert pasteboard snapshot is safe.
///
/// Only restore when the general pasteboard still holds exactly the text we
/// placed for paste. If it already differs, another writer (user copy or a
/// later insert) owns it — restoring would clobber them and can resurface a
/// prior dictation for the next paste.
pub(crate) fn should_restore_pasteboard_after_insert(
    inserted_text: &str,
    current_pasteboard_text: Option<&str>,
) -> bool {
    current_pasteboard_text == Some(inserted_text)
}

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
fn macos_accessibility_permission_granted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[cfg(not(target_os = "macos"))]
fn macos_accessibility_permission_granted() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn open_accessibility_settings() -> Result<(), String> {
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

/// Minimum gap between Accessibility Settings reopen attempts.
///
/// Paste attempts used to call `open` on every failed insert, which felt like a
/// permission loop when the Accessibility toggle looked on but TCC still denied
/// the current ad-hoc binary (`AXIsProcessTrusted` false after rebuild/re-sign).
#[cfg(target_os = "macos")]
const ACCESSIBILITY_SETTINGS_REOPEN_COOLDOWN: Duration = Duration::from_secs(60);

#[cfg(target_os = "macos")]
fn maybe_open_accessibility_settings() -> bool {
    use std::sync::Mutex;
    use std::time::Instant;

    static LAST_OPENED: Mutex<Option<Instant>> = Mutex::new(None);

    let mut guard = match LAST_OPENED.lock() {
        Ok(guard) => guard,
        Err(_) => {
            log::warn!("Failed to lock Accessibility settings reopen cooldown");
            return false;
        }
    };

    if guard.is_some_and(|last| last.elapsed() < ACCESSIBILITY_SETTINGS_REOPEN_COOLDOWN) {
        return false;
    }

    match open_accessibility_settings() {
        Ok(()) => {
            *guard = Some(Instant::now());
            true
        }
        Err(error) => {
            log::warn!("Failed to open Accessibility settings: {error}");
            false
        }
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

        let opened = if prompt_if_missing {
            maybe_open_accessibility_settings()
        } else {
            false
        };

        return FocusedFieldInsertPermissionStatus {
            granted: false,
            status: if opened {
                "Focused-field insertion needs Accessibility permission. Opened System Settings > Privacy & Security > Accessibility. Remove dicktaint if it is already listed, add /Applications/dicktaint.app again, enable it, then relaunch dicktaint."
                    .to_string()
            } else {
                "Focused-field insertion needs Accessibility permission. In System Settings > Privacy & Security > Accessibility, remove dicktaint if listed, add /Applications/dicktaint.app again, enable it, then relaunch dicktaint."
                    .to_string()
            },
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
#[cfg(target_os = "macos")]
fn write_text_to_general_pasteboard(
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
fn restore_general_pasteboard(
    pasteboard: &NSPasteboard,
    inserted_text: &str,
    snapshot: Option<String>,
) -> Result<(), String> {
    let current =
        unsafe { pasteboard.stringForType(NSPasteboardTypeString) }.map(|value| value.to_string());
    if !should_restore_pasteboard_after_insert(inserted_text, current.as_deref()) {
        return Ok(());
    }

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
fn post_keyboard_event(keycode: u16, key_down: bool, flags: CGEventFlags) -> Result<(), String> {
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
fn post_command_v_paste() -> Result<(), String> {
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

    // Serialize pasteboard ownership across concurrent frontend / IPC callers.
    let _guard = FOCUSED_FIELD_INSERT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Do not reopen Settings on every paste — only surface the status. Enabling
    // the setting (and the cooldown helper) is what prompts System Settings.
    let permission = focused_field_insert_permission_status(true, false);
    if !permission.granted {
        return Err(permission.status);
    }

    let (pasteboard, snapshot) = write_text_to_general_pasteboard(trimmed)?;
    let paste_result = post_command_v_paste();
    thread::sleep(PASTEBOARD_PASTE_SETTLE);
    let restore_result = restore_general_pasteboard(&pasteboard, trimmed, snapshot);

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

#[cfg(test)]
mod tests {
    use super::should_restore_pasteboard_after_insert;

    #[test]
    fn restores_only_when_pasteboard_still_holds_inserted_text() {
        assert!(should_restore_pasteboard_after_insert(
            "latest dictation",
            Some("latest dictation")
        ));
        assert!(!should_restore_pasteboard_after_insert(
            "latest dictation",
            Some("previous dictation")
        ));
        assert!(!should_restore_pasteboard_after_insert("latest dictation", None));
    }
}
