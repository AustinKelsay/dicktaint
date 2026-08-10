//! Dictation trigger canonicalize/normalize/resolve, runtime details, and hotkey registration.

use crate::state::{
    DEFAULT_DICTATION_TRIGGER, DictationTriggerPayload, LocalSettings, MAX_DICTATION_TRIGGER_LENGTH,
};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Mutex;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[cfg(target_os = "macos")]
use super::macos_fn::MacFnGlobalListener;

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

#[derive(Default)]
pub(crate) struct GlobalHotkeyState {
    registered_trigger: Mutex<Option<String>>,
    runtime_details: Mutex<TriggerRuntimeDetails>,
    #[cfg(target_os = "macos")]
    macos_fn_listener: Mutex<Option<MacFnGlobalListener>>,
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

fn canonicalize_trigger_modifier(token: &str) -> Option<&'static str> {
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

fn canonicalize_trigger_key(token: &str) -> Option<String> {
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
        #[cfg(not(target_os = "macos"))]
        {
            return Err(
                "Fn trigger is only supported on macOS. Use a modifier+key combo instead."
                    .to_string(),
            );
        }
        #[cfg(target_os = "macos")]
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

fn global_toggle_status(trigger: &str) -> String {
    format!("Press {trigger} anywhere to start or stop dictation.")
}

fn global_hold_status(trigger: &str) -> String {
    format!("Hold {trigger} anywhere to dictate, then release to transcribe.")
}

fn focused_window_hold_status(trigger: &str) -> String {
    format!(
        "Hold {trigger} to dictate while dicktaint is focused. Grant Input Monitoring for global hold-to-talk."
    )
}

fn fn_permission_hint() -> String {
    "System Settings > Privacy & Security > Input Monitoring: allow dicktaint (or Terminal while running tauri:dev), then relaunch dicktaint.".to_string()
}

fn set_trigger_runtime_details(
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

fn runtime_details_for_trigger(
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
fn shortcut_from_dictation_trigger(trigger: &str) -> Result<Shortcut, String> {
    let normalized = normalize_dictation_trigger(trigger)?;
    let accelerator = normalized
        .replace("CmdOrCtrl", "CommandOrControl")
        .replace("Cmd", "Command");
    Shortcut::from_str(&accelerator)
        .map_err(|e| format!("Could not parse hotkey '{normalized}' for global registration: {e}"))
}

fn set_registered_hotkey_state(
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

fn update_hotkey_state(
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
fn should_register_global_hotkey(trigger: &str) -> bool {
    trigger != "Fn"
}

#[cfg(not(target_os = "macos"))]
fn should_register_global_hotkey(_trigger: &str) -> bool {
    true
}

#[cfg(target_os = "macos")]
fn set_macos_fn_listener_enabled(
    app: &tauri::AppHandle,
    hotkey_state: &GlobalHotkeyState,
    enabled: bool,
) -> Result<(), String> {
    let mut guard = hotkey_state
        .macos_fn_listener
        .lock()
        .map_err(|_| "Failed to lock macOS Fn listener state".to_string())?;

    if enabled {
        // Always rebuild. Listen-only taps created before Input Monitoring is
        // granted (or before an ad-hoc CDHash is allowed) never receive
        // background Fn events until the tap is destroyed and recreated.
        if let Some(listener) = guard.as_ref() {
            listener.set_enabled(false);
        }
        *guard = None;
        *guard = Some(MacFnGlobalListener::new(app)?);
        if let Some(listener) = guard.as_ref() {
            listener.set_enabled(true);
        }
    } else if let Some(listener) = guard.take() {
        listener.set_enabled(false);
        drop(listener);
    }

    Ok(())
}

/**
 * Rebuilds the global Fn listener after the app becomes active again.
 *
 * Users typically toggle Input Monitoring in System Settings then return to
 * dicktaint; without a recreate, the old silent tap stays installed.
 */
#[cfg(target_os = "macos")]
pub(crate) fn refresh_macos_fn_listener_after_activation(
    app: &tauri::AppHandle,
    hotkey_state: &GlobalHotkeyState,
) -> Result<(), String> {
    use super::macos_fn::should_refresh_fn_listener_after_activation;

    use super::macos_fn::claim_fn_listener_activation_refresh_slot;

    let registered = current_registered_hotkey(hotkey_state)?;
    if !should_refresh_fn_listener_after_activation(registered.as_deref() == Some("Fn")) {
        return Ok(());
    }
    if !claim_fn_listener_activation_refresh_slot() {
        return Ok(());
    }

    match set_macos_fn_listener_enabled(app, hotkey_state, true) {
        Ok(()) => {
            let runtime =
                runtime_details_for_trigger(Some("Fn"), HotkeyDeliveryMode::GlobalHold);
            update_hotkey_state(hotkey_state, Some("Fn".to_string()), runtime)?;
            Ok(())
        }
        Err(error) => {
            log::warn!(
                "Failed to refresh global Fn listener after activation: {error}"
            );
            let runtime =
                runtime_details_for_trigger(Some("Fn"), HotkeyDeliveryMode::FocusedWindowHold);
            update_hotkey_state(hotkey_state, Some("Fn".to_string()), runtime)?;
            Err(error)
        }
    }
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



#[cfg(test)]
mod tests {
    use super::{
        default_dictation_trigger, focused_field_insert_enabled, normalize_dictation_trigger,
        onboarding_runtime_details, resolve_effective_dictation_trigger,
        runtime_details_for_trigger, HotkeyDeliveryMode,
    };
    use crate::state::LocalSettings;

    #[test]
    fn normalize_dictation_trigger_accepts_valid_combo() {
        assert_eq!(
            normalize_dictation_trigger("cmdorctrl + shift + d").unwrap(),
            "CmdOrCtrl+Shift+D".to_string()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn normalize_dictation_trigger_accepts_fn_key() {
        assert_eq!(normalize_dictation_trigger("fn").unwrap(), "Fn".to_string());
        assert_eq!(
            normalize_dictation_trigger("globe").unwrap(),
            "Fn".to_string()
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn normalize_dictation_trigger_rejects_fn_on_non_macos() {
        let err = normalize_dictation_trigger("fn").unwrap_err();
        assert!(err.contains("macOS"));
        assert!(normalize_dictation_trigger("globe").is_err());
    }

    #[test]
    fn normalize_dictation_trigger_rejects_fn_with_modifiers() {
        assert!(normalize_dictation_trigger("Shift+Fn").is_err());
    }

    #[test]
    fn normalize_dictation_trigger_rejects_missing_modifier() {
        assert!(normalize_dictation_trigger("D").is_err());
    }

    #[test]
    fn normalize_dictation_trigger_rejects_multiple_main_keys() {
        assert!(normalize_dictation_trigger("Ctrl+K+J").is_err());
    }

    #[test]
    fn resolve_effective_trigger_defaults_when_unset() {
        let settings = LocalSettings::default();
        assert_eq!(
            resolve_effective_dictation_trigger(&settings),
            Some(default_dictation_trigger())
        );
    }

    #[test]
    fn resolve_effective_trigger_honors_explicit_disable() {
        let settings = LocalSettings {
            dictation_trigger_enabled: Some(false),
            ..LocalSettings::default()
        };
        assert_eq!(resolve_effective_dictation_trigger(&settings), None);
    }

    #[test]
    fn resolve_effective_trigger_uses_saved_value() {
        let settings = LocalSettings {
            dictation_trigger: Some("CmdOrCtrl+Shift+K".to_string()),
            dictation_trigger_enabled: Some(true),
            ..LocalSettings::default()
        };
        assert_eq!(
            resolve_effective_dictation_trigger(&settings),
            Some("CmdOrCtrl+Shift+K".to_string())
        );
    }

    #[test]
    fn focused_field_insert_defaults_to_disabled() {
        let settings = LocalSettings::default();
        assert!(!focused_field_insert_enabled(&settings));
    }

    #[test]
    fn focused_field_insert_uses_explicit_enabled_setting() {
        let settings = LocalSettings {
            focused_field_insert_enabled: Some(true),
            ..LocalSettings::default()
        };
        assert!(focused_field_insert_enabled(&settings));
    }

    #[test]
    fn runtime_details_report_fn_permission_fallback() {
        let runtime =
            runtime_details_for_trigger(Some("Fn"), HotkeyDeliveryMode::FocusedWindowHold);
        assert_eq!(runtime.mode.as_str(), "focused-window-hold");
        assert!(runtime.status.contains("focused"));
        assert!(runtime.permission_hint.is_some());
    }

    #[test]
    fn onboarding_runtime_prefers_registered_global_fn_state() {
        let registered_runtime =
            runtime_details_for_trigger(Some("Fn"), HotkeyDeliveryMode::GlobalHold);
        let runtime = onboarding_runtime_details(Some("Fn"), Some("Fn"), Some(&registered_runtime));
        assert_eq!(runtime.mode.as_str(), "global-hold");
        assert!(runtime.status.contains("anywhere"));
    }

    #[test]
    fn onboarding_runtime_falls_back_when_fn_runtime_is_unknown() {
        let runtime = onboarding_runtime_details(Some("Fn"), None, None);
        assert_eq!(runtime.mode.as_str(), "focused-window-hold");
        assert!(runtime.status.contains("focused"));
        assert!(runtime.permission_hint.is_some());
    }
}
