//! Observational macOS privacy facts and explicit, allowlisted user actions.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MicrophonePermission {
    NotDetermined,
    Authorized,
    Denied,
    Restricted,
    Unavailable,
}

#[derive(Serialize)]
pub(crate) struct MacosPermissions {
    supported: bool,
    microphone: MicrophonePermission,
    accessibility: bool,
    input_monitoring: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PermissionPane {
    Microphone,
    Accessibility,
    InputMonitoring,
}
impl PermissionPane {
    fn settings_url(&self) -> &'static str {
        match self {
            Self::Microphone => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            Self::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            Self::InputMonitoring => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}
#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

#[tauri::command]
pub(crate) fn get_macos_permissions() -> MacosPermissions {
    #[cfg(target_os = "macos")]
    {
        MacosPermissions {
            supported: true,
            microphone: crate::audio::microphone_permission(),
            accessibility: unsafe { AXIsProcessTrusted() },
            input_monitoring: unsafe { CGPreflightListenEventAccess() },
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        MacosPermissions {
            supported: false,
            microphone: MicrophonePermission::Unavailable,
            accessibility: false,
            input_monitoring: false,
        }
    }
}

#[tauri::command]
pub(crate) async fn request_microphone_permission(
    app: tauri::AppHandle,
) -> Result<MacosPermissions, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if get_macos_permissions().microphone == MicrophonePermission::NotDetermined {
            // A timeout is still unconfirmed, never a denial. Return the actual OS facts.
            if let Err(error) = crate::audio::ensure_microphone_access_authorized(&app) {
                log::info!("{error}");
            }
        }
        get_macos_permissions()
    })
    .await
    .map_err(|e| format!("Microphone permission request failed: {e}"))
}

#[tauri::command]
pub(crate) fn open_macos_permission_settings(permission: PermissionPane) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if matches!(permission, PermissionPane::InputMonitoring) {
            // Explicit action registers this app in TCC, including fresh signing identities.
            unsafe {
                CGRequestListenEventAccess();
            }
        }
        let status = std::process::Command::new("open")
            .arg(permission.settings_url())
            .status()
            .map_err(|e| format!("Could not open System Settings: {e}"))?;
        if !status.success() {
            return Err("Could not open System Settings.".into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = permission;
        Err("macOS privacy settings are unavailable on this platform.".into())
    }
}

#[tauri::command]
pub(crate) fn recheck_macos_permissions(app: tauri::AppHandle) -> Result<MacosPermissions, String> {
    #[cfg(target_os = "macos")]
    {
        use tauri::Manager;
        if let Err(error) = crate::hotkey_overlay::refresh_macos_fn_listener_after_activation(
            &app,
            app.state::<crate::hotkey_overlay::GlobalHotkeyState>()
                .inner(),
        ) {
            // Optional background Fn failure does not invalidate microphone facts.
            log::info!("Background Fn recheck: {error}");
        }
    }
    Ok(get_macos_permissions())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permission_contract_distinguishes_unrequested_and_restricted_microphone() {
        assert_eq!(
            serde_json::to_string(&MicrophonePermission::NotDetermined).unwrap(),
            "\"not_determined\""
        );
        assert_eq!(
            serde_json::to_string(&MicrophonePermission::Restricted).unwrap(),
            "\"restricted\""
        );
        assert!(serde_json::from_str::<PermissionPane>("\"https://example.com\"").is_err());
        let pane: PermissionPane = serde_json::from_str("\"microphone\"").unwrap();
        assert_eq!(
            pane.settings_url(),
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        );
    }
}
