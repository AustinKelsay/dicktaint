use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::str::FromStr;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const WHISPER_SAMPLE_RATE: u32 = 16_000;
const APP_SETTINGS_DIR: &str = ".dicktaint";
const APP_SETTINGS_FILE: &str = "dictation-settings.json";
const APP_MODELS_DIR: &str = "whisper-models";
const DEFAULT_WHISPER_CLI_PATH: &str = "whisper-cli";
#[cfg(target_os = "macos")]
const DEFAULT_DICTATION_TRIGGER: &str = "Fn";
#[cfg(not(target_os = "macos"))]
const DEFAULT_DICTATION_TRIGGER: &str = "CmdOrCtrl+Shift+D";
const MAX_DICTATION_TRIGGER_LENGTH: usize = 64;
const DICTATION_HOTKEY_EVENT: &str = "dictation:hotkey-triggered";
const DICTATION_STATE_EVENT: &str = "dictation:state-changed";
const WHISPER_CPP_SETUP_URL: &str = "https://github.com/ggml-org/whisper.cpp#quick-start";
const START_HIDDEN_ENV: &str = "DICKTAINT_START_HIDDEN";
const FN_HOTKEY_STATE_EVENT: &str = "dicktaint://fn-state";
const PILL_WINDOW_LABEL_PREFIX: &str = "pill";
const PILL_WINDOW_WIDTH: f64 = 278.0;
const PILL_WINDOW_HEIGHT: f64 = 40.0;
const PILL_WINDOW_BOTTOM_MARGIN: i32 = 18;
const MAX_PILL_WINDOWS: usize = 6;
const MIN_TRANSCRIBE_SECONDS: f32 = 0.30;
const MIN_SPEECH_RMS: f32 = 0.003;
const MIN_SPEECH_PEAK: f32 = 0.020;
const SILENCE_WINDOW_MS: u32 = 20;
const SILENCE_GATE_ABS_MEAN: f32 = 0.008;
const SILENCE_TRIM_PAD_MS: u32 = 160;
const LOW_CONFIDENCE_RETRY_SECONDS: f32 = 6.0;

#[derive(Clone, Serialize)]
struct DictationStatePayload {
    state: String,
    error: Option<String>,
    transcript: Option<String>,
}

#[derive(Clone)]
struct AppConfig {
    whisper_model_path_override: Option<String>,
    whisper_cli_path_override: Option<String>,
    bundled_whisper_cli_path: Option<String>,
}

#[derive(Default)]
struct DictationState {
    active_recording: Mutex<Option<ActiveRecording>>,
}

#[derive(Serialize, Clone, Copy)]
struct FnHotkeyStatePayload {
    pressed: bool,
}

#[derive(Clone, Copy)]
struct WhisperModelSpec {
    id: &'static str,
    display_name: &'static str,
    whisper_ref: &'static str,
    file_name: &'static str,
    approx_size_gb: f32,
    min_ram_gb: u64,
    recommended_ram_gb: u64,
    speed_note: &'static str,
    quality_note: &'static str,
}

const WHISPER_MODEL_CATALOG: [WhisperModelSpec; 12] = [
    WhisperModelSpec {
        id: "tiny-en",
        display_name: "Whisper Tiny (English)",
        whisper_ref: "tiny.en",
        file_name: "ggml-tiny.en.bin",
        approx_size_gb: 0.08,
        min_ram_gb: 4,
        recommended_ram_gb: 8,
        speed_note: "Fastest",
        quality_note: "Lowest accuracy",
    },
    WhisperModelSpec {
        id: "tiny",
        display_name: "Whisper Tiny (Multilingual)",
        whisper_ref: "tiny",
        file_name: "ggml-tiny.bin",
        approx_size_gb: 0.15,
        min_ram_gb: 6,
        recommended_ram_gb: 8,
        speed_note: "Very fast",
        quality_note: "Low accuracy",
    },
    WhisperModelSpec {
        id: "base-en",
        display_name: "Whisper Base (English)",
        whisper_ref: "base.en",
        file_name: "ggml-base.en.bin",
        approx_size_gb: 0.15,
        min_ram_gb: 6,
        recommended_ram_gb: 10,
        speed_note: "Fast",
        quality_note: "Balanced",
    },
    WhisperModelSpec {
        id: "base",
        display_name: "Whisper Base (Multilingual)",
        whisper_ref: "base",
        file_name: "ggml-base.bin",
        approx_size_gb: 0.29,
        min_ram_gb: 8,
        recommended_ram_gb: 12,
        speed_note: "Fast",
        quality_note: "Balanced multilingual",
    },
    WhisperModelSpec {
        id: "small-en",
        display_name: "Whisper Small (English)",
        whisper_ref: "small.en",
        file_name: "ggml-small.en.bin",
        approx_size_gb: 0.46,
        min_ram_gb: 8,
        recommended_ram_gb: 16,
        speed_note: "Medium",
        quality_note: "Better accuracy",
    },
    WhisperModelSpec {
        id: "small",
        display_name: "Whisper Small (Multilingual)",
        whisper_ref: "small",
        file_name: "ggml-small.bin",
        approx_size_gb: 0.93,
        min_ram_gb: 10,
        recommended_ram_gb: 18,
        speed_note: "Medium",
        quality_note: "Better multilingual accuracy",
    },
    WhisperModelSpec {
        id: "medium-en",
        display_name: "Whisper Medium (English)",
        whisper_ref: "medium.en",
        file_name: "ggml-medium.en.bin",
        approx_size_gb: 1.5,
        min_ram_gb: 16,
        recommended_ram_gb: 24,
        speed_note: "Slowest in starter set",
        quality_note: "Best accuracy in starter set",
    },
    WhisperModelSpec {
        id: "medium",
        display_name: "Whisper Medium (Multilingual)",
        whisper_ref: "medium",
        file_name: "ggml-medium.bin",
        approx_size_gb: 1.5,
        min_ram_gb: 18,
        recommended_ram_gb: 28,
        speed_note: "Slower",
        quality_note: "Strong multilingual accuracy",
    },
    WhisperModelSpec {
        id: "large-v1",
        display_name: "Whisper Large v1",
        whisper_ref: "large-v1",
        file_name: "ggml-large-v1.bin",
        approx_size_gb: 2.9,
        min_ram_gb: 24,
        recommended_ram_gb: 32,
        speed_note: "Heavy",
        quality_note: "High accuracy",
    },
    WhisperModelSpec {
        id: "large-v2",
        display_name: "Whisper Large v2",
        whisper_ref: "large-v2",
        file_name: "ggml-large-v2.bin",
        approx_size_gb: 2.9,
        min_ram_gb: 24,
        recommended_ram_gb: 32,
        speed_note: "Heavy",
        quality_note: "High accuracy",
    },
    WhisperModelSpec {
        id: "large-v3",
        display_name: "Whisper Large v3",
        whisper_ref: "large-v3",
        file_name: "ggml-large-v3.bin",
        approx_size_gb: 3.1,
        min_ram_gb: 32,
        recommended_ram_gb: 48,
        speed_note: "Heaviest",
        quality_note: "Top accuracy",
    },
    WhisperModelSpec {
        id: "turbo",
        display_name: "Whisper Turbo",
        whisper_ref: "turbo",
        file_name: "ggml-large-v3-turbo.bin",
        approx_size_gb: 1.62,
        min_ram_gb: 20,
        recommended_ram_gb: 32,
        speed_note: "Fast large-class",
        quality_note: "Great quality/speed tradeoff",
    },
];

#[derive(Default, Serialize, Deserialize, Clone)]
struct LocalSettings {
    selected_model_id: Option<String>,
    selected_model_path: Option<String>,
    dictation_trigger: Option<String>,
}

struct LocalModelState {
    settings_path: PathBuf,
    models_dir: PathBuf,
    settings: Arc<Mutex<LocalSettings>>,
}

#[derive(Default)]
struct GlobalHotkeyState {
    registered_trigger: Mutex<Option<String>>,
    #[cfg(target_os = "macos")]
    macos_fn_listener: Mutex<Option<MacFnGlobalListener>>,
}

#[cfg(target_os = "macos")]
type CFAllocatorRef = *mut c_void;
#[cfg(target_os = "macos")]
type CFMachPortRef = *mut c_void;
#[cfg(target_os = "macos")]
type CFRunLoopRef = *mut c_void;
#[cfg(target_os = "macos")]
type CFRunLoopSourceRef = *mut c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const c_void;
#[cfg(target_os = "macos")]
type CGEventRef = *const c_void;
#[cfg(target_os = "macos")]
type CGEventTapProxy = *const c_void;
#[cfg(target_os = "macos")]
type CGEventMask = u64;
#[cfg(target_os = "macos")]
type CGEventFlags = u64;

#[cfg(target_os = "macos")]
const CG_EVENT_TAP_LOCATION_SESSION: u32 = 1;
#[cfg(target_os = "macos")]
const CG_EVENT_TAP_PLACEMENT_HEAD_INSERT: u32 = 0;
#[cfg(target_os = "macos")]
const CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
#[cfg(target_os = "macos")]
const CG_EVENT_TYPE_FLAGS_CHANGED: u32 = 12;

#[cfg(target_os = "macos")]
const MACOS_FN_FLAG_MASK: CGEventFlags = 1 << 23;
#[cfg(target_os = "macos")]
const MACOS_NON_FN_MODIFIER_MASK: CGEventFlags = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20);

#[cfg(target_os = "macos")]
type MacFnEventTapCallback =
    unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

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

#[cfg(target_os = "macos")]
struct MacFnCallbackContext {
    app: tauri::AppHandle,
    enabled: AtomicBool,
    fn_down: AtomicBool,
}

#[cfg(target_os = "macos")]
struct MacFnGlobalListener {
    tap: CFMachPortRef,
    source: CFRunLoopSourceRef,
    callback_ctx: Arc<MacFnCallbackContext>,
    callback_ctx_raw: *const MacFnCallbackContext,
}

#[cfg(target_os = "macos")]
unsafe impl Send for MacFnGlobalListener {}
#[cfg(target_os = "macos")]
unsafe impl Sync for MacFnGlobalListener {}

#[cfg(target_os = "macos")]
impl MacFnGlobalListener {
    fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let callback_ctx = Arc::new(MacFnCallbackContext {
            app: app.clone(),
            enabled: AtomicBool::new(false),
            fn_down: AtomicBool::new(false),
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

        let source = unsafe { CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) };
        if source.is_null() {
            unsafe {
                CFMachPortInvalidate(tap);
                CFRelease(tap as *const c_void);
                drop(Arc::from_raw(callback_ctx_raw));
            }
            return Err("Failed to create macOS run loop source for global Fn listener.".to_string());
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

    fn set_enabled(&self, enabled: bool) {
        self.callback_ctx.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            self.callback_ctx.fn_down.store(false, Ordering::SeqCst);
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
    if event.is_null() || user_info.is_null() || event_type != CG_EVENT_TYPE_FLAGS_CHANGED {
        return event;
    }

    let callback_ctx = &*(user_info as *const MacFnCallbackContext);
    if !callback_ctx.enabled.load(Ordering::Relaxed) {
        return event;
    }

    let flags = CGEventGetFlags(event);
    let fn_down = (flags & MACOS_FN_FLAG_MASK) != 0;
    let was_fn_down = callback_ctx.fn_down.swap(fn_down, Ordering::Relaxed);

    let has_non_fn_modifiers = (flags & MACOS_NON_FN_MODIFIER_MASK) != 0;
    if fn_down && !was_fn_down && !has_non_fn_modifiers {
        if let Err(error) = callback_ctx.app.emit(DICTATION_HOTKEY_EVENT, ()) {
            log::warn!("Failed to emit global Fn hotkey event: {error}");
        }
    }

    event
}

#[derive(Serialize)]
struct DeviceProfile {
    total_memory_gb: u64,
    logical_cpu_cores: usize,
    architecture: String,
    os: String,
}

#[derive(Serialize)]
struct DictationModelOption {
    id: String,
    display_name: String,
    whisper_ref: String,
    file_name: String,
    path: String,
    installed: bool,
    likely_runnable: bool,
    recommended: bool,
    approx_size_gb: f32,
    min_ram_gb: u64,
    recommended_ram_gb: u64,
    speed_note: String,
    quality_note: String,
}

#[derive(Serialize)]
struct DictationOnboardingPayload {
    onboarding_required: bool,
    selected_model_id: Option<String>,
    selected_model_path: Option<String>,
    selected_model_exists: bool,
    dictation_trigger: Option<String>,
    default_dictation_trigger: String,
    whisper_cli_available: bool,
    whisper_cli_path: String,
    models_dir: String,
    device: DeviceProfile,
    models: Vec<DictationModelOption>,
}

#[derive(Serialize)]
struct DictationTriggerPayload {
    trigger: Option<String>,
    default_trigger: String,
}

#[derive(Serialize)]
struct DictationModelSelection {
    selected_model_id: String,
    selected_model_path: String,
    installed: bool,
}

#[derive(Serialize)]
struct DictationModelDeletion {
    deleted_model_id: String,
    selected_model_id: Option<String>,
    selected_model_path: Option<String>,
}

struct ActiveRecording {
    stop_tx: mpsc::Sender<()>,
    thread_handle: thread::JoinHandle<()>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

#[derive(Clone, Copy)]
struct WhisperDecodeProfile {
    beam_size: u8,
    best_of: u8,
}

const FAST_DECODE_PROFILE: WhisperDecodeProfile = WhisperDecodeProfile {
    beam_size: 2,
    best_of: 2,
};

const ACCURATE_DECODE_PROFILE: WhisperDecodeProfile = WhisperDecodeProfile {
    beam_size: 5,
    best_of: 5,
};

fn parse_truthy_env(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    !matches!(normalized.as_str(), "" | "0" | "false" | "no" | "off")
}

fn should_start_hidden() -> bool {
    std::env::var(START_HIDDEN_ENV)
        .map(|value| parse_truthy_env(&value))
        .unwrap_or(false)
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn create_pill_overlay_window_for_monitor(
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
    let width_i = PILL_WINDOW_WIDTH as i32;
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
            .inner_size(PILL_WINDOW_WIDTH, PILL_WINDOW_HEIGHT)
            .position(x as f64, y as f64)
            .build()
            .map_err(|e| format!("Failed to create overlay window '{label}': {e}"))?;

    let _ = window.set_ignore_cursor_events(true);
    let _ = window.set_always_on_top(true);
    let _ = window.set_visible_on_all_workspaces(true);
    Ok(())
}

#[cfg(target_os = "macos")]
fn create_pill_overlay_windows(app: &tauri::AppHandle) -> Result<(), String> {
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
fn create_pill_overlay_windows(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn register_fn_global_hotkey_monitor(app: &tauri::AppHandle) -> Result<(), String> {
    let app_handle = app.clone();
    let fn_key_down = Arc::new(AtomicBool::new(false));
    let fn_key_down_ref = Arc::clone(&fn_key_down);
    let handler = RcBlock::new(move |event_ptr: NonNull<NSEvent>| {
        // SAFETY: NSEvent monitor callback provides a valid NSEvent pointer for callback lifetime.
        let event = unsafe { event_ptr.as_ref() };
        let function_down = event
            .modifierFlags()
            .contains(NSEventModifierFlags::Function);
        // Emit only on edge transitions so frontend start/stop handling stays idempotent.
        let previous = fn_key_down_ref.swap(function_down, Ordering::SeqCst);
        if previous != function_down {
            let _ = app_handle.emit(
                FN_HOTKEY_STATE_EVENT,
                FnHotkeyStatePayload {
                    pressed: function_down,
                },
            );
        }
    });

    let monitor =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::FlagsChanged, &handler)
            .ok_or_else(|| {
                "Failed to register global fn key monitor on macOS. \
Allow Input Monitoring/Accessibility for this app or terminal and retry."
                    .to_string()
            })?;

    // Keep monitor and callback alive for process lifetime.
    std::mem::forget(handler);
    std::mem::forget(monitor);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn register_fn_global_hotkey_monitor(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

fn resolve_whisper_model_path(path: Option<&str>) -> Result<PathBuf, String> {
    let raw = path
    .map(str::trim)
    .filter(|v| !v.is_empty())
    .ok_or_else(|| {
      "WHISPER_MODEL_PATH is not set. Point it to a local GGML Whisper model file (example: ggml-base.en.bin).".to_string()
    })?;

    let model_path = PathBuf::from(raw);
    if !model_path.exists() {
        return Err(format!(
            "WHISPER_MODEL_PATH file not found: {}",
            model_path.display()
        ));
    }

    Ok(model_path)
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

fn normalize_dictation_trigger(trigger: &str) -> Result<String, String> {
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

fn dictation_trigger_payload(settings: &LocalSettings) -> DictationTriggerPayload {
    let trigger = settings
        .dictation_trigger
        .as_deref()
        .and_then(|value| normalize_dictation_trigger(value).ok());

    DictationTriggerPayload {
        trigger,
        default_trigger: DEFAULT_DICTATION_TRIGGER.to_string(),
    }
}

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

fn current_registered_hotkey(hotkey_state: &GlobalHotkeyState) -> Result<Option<String>, String> {
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
        if guard.is_none() {
            *guard = Some(MacFnGlobalListener::new(app)?);
        }
    }

    if let Some(listener) = guard.as_ref() {
        listener.set_enabled(enabled);
    }

    Ok(())
}

fn apply_registered_hotkey(
    app: &tauri::AppHandle,
    hotkey_state: &GlobalHotkeyState,
    trigger: Option<&str>,
) -> Result<(), String> {
    let next = match trigger.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Some(normalize_dictation_trigger(value)?),
        None => None,
    };
    let previous = current_registered_hotkey(hotkey_state)?;

    if previous == next {
        return Ok(());
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
        #[cfg(target_os = "macos")]
        if next_trigger == "Fn" {
            if let Err(error) = set_macos_fn_listener_enabled(app, hotkey_state, true) {
                log::warn!(
                    "Global Fn listener unavailable; falling back to in-app Fn hotkey handling: {error}"
                );
            }
        }

        if should_register_global_hotkey(next_trigger) {
            let next_shortcut = shortcut_from_dictation_trigger(next_trigger)?;
            if let Err(error) = app.global_shortcut().register(next_shortcut) {
                if let Some(previous_trigger) = previous.as_deref() {
                    if should_register_global_hotkey(previous_trigger) {
                        if let Ok(previous_shortcut) = shortcut_from_dictation_trigger(previous_trigger)
                        {
                            if let Err(recovery_error) =
                                app.global_shortcut().register(previous_shortcut)
                            {
                                set_registered_hotkey_state(hotkey_state, None)?;
                                return Err(format!(
                                    "Could not register global hotkey '{next_trigger}': {error}. Also failed to restore previous hotkey '{previous_trigger}': {recovery_error}"
                                ));
                            }
                            set_registered_hotkey_state(
                                hotkey_state,
                                Some(previous_trigger.to_string()),
                            )?;
                        } else {
                            set_registered_hotkey_state(hotkey_state, None)?;
                        }
                    } else {
                        set_registered_hotkey_state(hotkey_state, Some(previous_trigger.to_string()))?;
                    }
                } else {
                    set_registered_hotkey_state(hotkey_state, None)?;
                }
                return Err(format!(
                    "Could not register global hotkey '{next_trigger}': {error}"
                ));
            }
        }
    }

    set_registered_hotkey_state(hotkey_state, next)
}

fn resolve_whisper_cli_path(override_path: Option<&str>, bundled_path: Option<&str>) -> String {
    let preferred = if let Some(path) = override_path.map(str::trim).filter(|v| !v.is_empty()) {
        path.to_string()
    } else if let Some(path) = bundled_path.map(str::trim).filter(|v| !v.is_empty()) {
        path.to_string()
    } else {
        DEFAULT_WHISPER_CLI_PATH.to_string()
    };

    detect_whisper_cli_path(&preferred).unwrap_or(preferred)
}

fn ensure_whisper_cli_available(whisper_cli_path: &str) -> Result<(), String> {
    let executable = validate_whisper_cli_candidate(whisper_cli_path).map_err(|detail| {
        format!(
            "Could not execute '{whisper_cli_path}': {detail}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
        )
    })?;
    let output = run_help_probe(&executable).map_err(|e| {
        format!(
            "Could not execute '{whisper_cli_path}' (resolved to {}): {e}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH.",
            executable.display()
        )
    })?;
    if help_probe_looks_like_whisper_cli(&output) {
        return Ok(());
    }

    let probe_summary = help_probe_summary(&output);
    Err(format!(
        "Could not execute '{whisper_cli_path}' (resolved to {}): probe exited with status {} and did not return recognizable whisper-cli help output ({probe_summary}). Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH.",
        executable.display(),
        output.status
    ))
}

fn can_execute_command(executable: &str) -> bool {
    let path = match validate_whisper_cli_candidate(executable) {
        Ok(path) => path,
        Err(_) => return false,
    };
    run_help_probe(&path)
        .map(|output| help_probe_looks_like_whisper_cli(&output))
        .unwrap_or(false)
}

fn run_help_probe(executable: &Path) -> Result<Output, std::io::Error> {
    Command::new(executable).arg("--help").output()
}

fn help_probe_looks_like_whisper_cli(output: &Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    whisper_help_text_looks_valid(&stdout, &stderr)
}

fn help_probe_summary(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(line) = stderr.lines().map(str::trim).find(|line| !line.is_empty()) {
        return line.to_string();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = stdout.lines().map(str::trim).find(|line| !line.is_empty()) {
        return line.to_string();
    }

    "no output".to_string()
}

fn whisper_help_text_looks_valid(stdout: &str, stderr: &str) -> bool {
    let normalized = format!("{stdout}\n{stderr}").trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    if normalized.contains("placeholder")
        && normalized.contains("replace")
        && normalized.contains("whisper-cli")
    {
        return false;
    }

    let has_usage = normalized.contains("usage") || normalized.contains("options");
    let has_model_flag = normalized.contains("--model")
        || normalized.contains("\n-m ")
        || normalized.contains(" -m ");
    has_usage && has_model_flag
}

fn validate_whisper_cli_candidate(candidate: &str) -> Result<PathBuf, String> {
    let resolved_path = resolve_command_path(candidate).ok_or_else(|| {
        if is_explicit_path(candidate) {
            format!(
                "whisper-cli file not found at {}",
                Path::new(candidate).display()
            )
        } else {
            format!("whisper-cli command '{candidate}' was not found in PATH")
        }
    })?;

    let metadata = fs::metadata(&resolved_path).map_err(|e| {
        format!(
            "failed to read file metadata for {}: {e}",
            resolved_path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "{} exists but is not a file",
            resolved_path.display()
        ));
    }

    #[cfg(unix)]
    {
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!(
                "{} is not executable (missing execute permission bits)",
                resolved_path.display()
            ));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let extension = resolved_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();
        let has_executable_extension = matches!(extension.as_str(), "exe" | "com" | "bat" | "cmd");
        if !has_executable_extension {
            return Err(format!(
                "{} is not an executable file (expected .exe/.com/.bat/.cmd)",
                resolved_path.display()
            ));
        }
    }

    Ok(resolved_path)
}

fn resolve_command_path(candidate: &str) -> Option<PathBuf> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }

    if is_explicit_path(trimmed) {
        return Some(PathBuf::from(trimmed));
    }

    let path_var = std::env::var_os("PATH")?;

    #[cfg(target_os = "windows")]
    {
        let has_extension = Path::new(trimmed).extension().is_some();
        let extensions = std::env::var("PATHEXT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                if value.starts_with('.') {
                    value.to_string()
                } else {
                    format!(".{value}")
                }
            })
            .collect::<Vec<_>>();

        for dir in std::env::split_paths(&path_var) {
            if has_extension {
                let candidate_path = dir.join(trimmed);
                if candidate_path.exists() {
                    return Some(candidate_path);
                }
                continue;
            }

            for extension in &extensions {
                let candidate_path = dir.join(format!("{trimmed}{extension}"));
                if candidate_path.exists() {
                    return Some(candidate_path);
                }
            }
        }
        return None;
    }

    #[cfg(not(target_os = "windows"))]
    {
        for dir in std::env::split_paths(&path_var) {
            let candidate_path = dir.join(trimmed);
            if candidate_path.exists() {
                return Some(candidate_path);
            }
        }
    }

    None
}

fn is_explicit_path(value: &str) -> bool {
    Path::new(value).is_absolute() || value.contains('/') || value.contains('\\')
}

fn preferred_whisper_cli_names() -> Vec<String> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let mut names = Vec::<String>::new();

    if os == "windows" {
        names.push(format!("whisper-cli-{arch}-pc-windows-msvc.exe"));
        names.push("whisper-cli.exe".to_string());
    } else if os == "macos" {
        names.push(format!("whisper-cli-{arch}-apple-darwin"));
        names.push("whisper-cli".to_string());
    } else if os == "linux" {
        names.push(format!("whisper-cli-{arch}-unknown-linux-gnu"));
        names.push("whisper-cli".to_string());
    } else {
        names.push("whisper-cli".to_string());
    }

    names
}

fn find_whisper_cli_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in preferred_whisper_cli_names() {
        let preferred = dir.join(name);
        if preferred.is_file() {
            return Some(preferred);
        }
    }
    None
}

fn resolve_bundled_whisper_cli_path(app: &tauri::AppHandle) -> Option<String> {
    let mut candidate_dirs = Vec::<PathBuf>::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidate_dirs.push(resource_dir.clone());
        candidate_dirs.push(resource_dir.join("bin"));
        candidate_dirs.push(resource_dir.join("binaries"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidate_dirs.push(parent.to_path_buf());
            if cfg!(target_os = "macos") {
                candidate_dirs.push(parent.join("../Resources"));
                candidate_dirs.push(parent.join("../Resources/bin"));
            }
        }
    }

    // In tauri:dev, sidecar binaries usually live in src-tauri/binaries.
    candidate_dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries"));

    let mut deduped = Vec::<PathBuf>::new();
    for dir in candidate_dirs {
        if !deduped.iter().any(|seen| seen == &dir) {
            deduped.push(dir);
        }
    }

    for dir in deduped {
        if let Some(path) = find_whisper_cli_in_dir(&dir) {
            return Some(path.to_string_lossy().to_string());
        }
    }

    None
}

fn local_dev_sidecar_candidates() -> Vec<String> {
    let binaries_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    preferred_whisper_cli_names()
        .into_iter()
        .map(|name| binaries_dir.join(name).to_string_lossy().to_string())
        .collect()
}

fn candidate_whisper_cli_paths(configured_path: &str) -> Vec<String> {
    let mut candidates = Vec::<String>::new();

    if !configured_path.trim().is_empty() {
        candidates.push(configured_path.trim().to_string());
    }
    candidates.extend(local_dev_sidecar_candidates());
    if configured_path.trim() != DEFAULT_WHISPER_CLI_PATH {
        candidates.push(DEFAULT_WHISPER_CLI_PATH.to_string());
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push("/opt/homebrew/bin/whisper-cli".to_string());
        candidates.push("/usr/local/bin/whisper-cli".to_string());
        candidates.push("/opt/homebrew/opt/whisper-cpp/bin/whisper-cli".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        candidates.push("/usr/local/bin/whisper-cli".to_string());
        candidates.push("/usr/bin/whisper-cli".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        candidates.push("C:\\Program Files\\whisper.cpp\\whisper-cli.exe".to_string());
        candidates.push("C:\\Program Files (x86)\\whisper.cpp\\whisper-cli.exe".to_string());
    }

    let mut deduped = Vec::new();
    for candidate in candidates {
        if !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn detect_whisper_cli_path(configured_path: &str) -> Option<String> {
    candidate_whisper_cli_paths(configured_path)
        .into_iter()
        .find(|candidate| can_execute_command(candidate))
}

fn resolve_local_paths(base_data_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let app_dir = base_data_dir.join(APP_SETTINGS_DIR);
    let models_dir = app_dir.join(APP_MODELS_DIR);
    let settings_path = app_dir.join(APP_SETTINGS_FILE);

    fs::create_dir_all(&app_dir).map_err(|e| {
        format!(
            "Failed to create local app settings directory {}: {e}",
            app_dir.display()
        )
    })?;
    fs::create_dir_all(&models_dir).map_err(|e| {
        format!(
            "Failed to create local model directory {}: {e}",
            models_dir.display()
        )
    })?;

    Ok((models_dir, settings_path))
}

fn load_local_settings(settings_path: &Path) -> LocalSettings {
    let raw = match fs::read_to_string(settings_path) {
        Ok(value) => value,
        Err(_) => return LocalSettings::default(),
    };

    match serde_json::from_str::<LocalSettings>(&raw) {
        Ok(settings) => settings,
        Err(error) => {
            log::warn!(
                "load_local_settings: failed to parse LocalSettings from {}: {}",
                settings_path.display(),
                error
            );
            LocalSettings::default()
        }
    }
}

fn save_local_settings(settings_path: &Path, settings: &LocalSettings) -> Result<(), String> {
    let parent = settings_path.parent().ok_or_else(|| {
        format!(
            "Failed to determine settings directory for {}",
            settings_path.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|e| {
        format!(
            "Failed to create settings directory {}: {e}",
            parent.display()
        )
    })?;

    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize dictation settings: {e}"))?;

    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let target_name = settings_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dictation-settings.json");
    // Write-then-rename keeps settings updates atomic across crashes/interruption.
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        target_name,
        std::process::id(),
        timestamp_nanos
    ));

    let mut temp_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|e| {
            format!(
                "Failed to create temp settings file {}: {e}",
                temp_path.display()
            )
        })?;
    temp_file.write_all(serialized.as_bytes()).map_err(|e| {
        format!(
            "Failed to write temp settings file {}: {e}",
            temp_path.display()
        )
    })?;
    temp_file.flush().map_err(|e| {
        format!(
            "Failed to flush temp settings file {}: {e}",
            temp_path.display()
        )
    })?;
    temp_file.sync_all().map_err(|e| {
        format!(
            "Failed to sync temp settings file {}: {e}",
            temp_path.display()
        )
    })?;
    drop(temp_file);

    fs::rename(&temp_path, settings_path).map_err(|e| {
        format!(
            "Failed to replace dictation settings file {} with temp file {}: {e}",
            settings_path.display(),
            temp_path.display()
        )
    })?;

    Ok(())
}

fn whisper_model_catalog() -> &'static [WhisperModelSpec] {
    &WHISPER_MODEL_CATALOG
}

fn find_whisper_model_spec(id: &str) -> Option<WhisperModelSpec> {
    whisper_model_catalog()
        .iter()
        .copied()
        .find(|spec| spec.id == id)
}

fn total_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return raw.parse::<u64>().ok();
    }

    #[cfg(target_os = "linux")]
    {
        let content = fs::read_to_string("/proc/meminfo").ok()?;
        let line = content
            .lines()
            .find(|entry| entry.starts_with("MemTotal:"))?;
        let kib = line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u64>().ok())?;
        return Some(kib.saturating_mul(1024));
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8(output.stdout)
            .ok()?
            .trim_matches(|c| c == '\r' || c == '\n' || c == ' ')
            .to_string();
        return value.parse::<u64>().ok();
    }

    #[allow(unreachable_code)]
    None
}

fn system_memory_gb() -> u64 {
    let total_bytes = total_memory_bytes().unwrap_or(8 * 1_073_741_824);
    (((total_bytes as f64) / 1_073_741_824.0).round() as u64).max(1)
}

fn build_device_profile() -> DeviceProfile {
    let logical_cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    DeviceProfile {
        total_memory_gb: system_memory_gb(),
        logical_cpu_cores,
        architecture: std::env::consts::ARCH.to_string(),
        os: std::env::consts::OS.to_string(),
    }
}

fn model_path_for_spec(models_dir: &Path, spec: WhisperModelSpec) -> PathBuf {
    models_dir.join(spec.file_name)
}

fn model_fit_level(spec: WhisperModelSpec, total_memory_gb: u64) -> u8 {
    if total_memory_gb >= spec.recommended_ram_gb {
        2
    } else if total_memory_gb >= spec.min_ram_gb {
        1
    } else {
        0
    }
}

fn pick_recommended_model_id(total_memory_gb: u64) -> Option<&'static str> {
    // For MVP dictation responsiveness, prefer practical quality/speed picks over largest runnable.
    // Runtime is currently English-first (`-l en`), so `.en` models are prioritized before multilingual.
    const PRACTICAL_ORDER: [&str; 7] = [
        "turbo", "base-en", "small-en", "tiny-en", "base", "small", "tiny",
    ];

    for id in PRACTICAL_ORDER {
        if let Some(spec) = find_whisper_model_spec(id) {
            if total_memory_gb >= spec.min_ram_gb {
                return Some(spec.id);
            }
        }
    }

    whisper_model_catalog()
        .iter()
        .copied()
        .filter(|spec| model_fit_level(*spec, total_memory_gb) > 0)
        .max_by(|a, b| {
            // Fallback only if practical presets are unavailable.
            let a_key = (
                model_fit_level(*a, total_memory_gb),
                a.recommended_ram_gb,
                a.approx_size_gb.to_bits(),
            );
            let b_key = (
                model_fit_level(*b, total_memory_gb),
                b.recommended_ram_gb,
                b.approx_size_gb.to_bits(),
            );
            a_key.cmp(&b_key)
        })
        .map(|spec| spec.id)
}

fn build_model_options(
    models_dir: &Path,
    total_memory_gb: u64,
    selected_model_id: Option<&str>,
) -> Vec<DictationModelOption> {
    let recommended_model_id = pick_recommended_model_id(total_memory_gb);

    whisper_model_catalog()
        .iter()
        .map(|spec| {
            let path = model_path_for_spec(models_dir, *spec);
            let installed = path.exists();
            let likely_runnable = total_memory_gb >= spec.min_ram_gb;
            let recommended = recommended_model_id.is_some_and(|id| id == spec.id);
            let is_selected = selected_model_id.is_some_and(|id| id == spec.id);

            DictationModelOption {
                id: spec.id.to_string(),
                display_name: if is_selected {
                    format!("{} (Selected)", spec.display_name)
                } else {
                    spec.display_name.to_string()
                },
                whisper_ref: spec.whisper_ref.to_string(),
                file_name: spec.file_name.to_string(),
                path: path.to_string_lossy().to_string(),
                installed,
                likely_runnable,
                recommended,
                approx_size_gb: spec.approx_size_gb,
                min_ram_gb: spec.min_ram_gb,
                recommended_ram_gb: spec.recommended_ram_gb,
                speed_note: spec.speed_note.to_string(),
                quality_note: spec.quality_note.to_string(),
            }
        })
        .collect()
}

fn pick_best_installed_model(
    models_dir: &Path,
    total_memory_gb: u64,
    exclude_model_id: Option<&str>,
) -> Option<(WhisperModelSpec, PathBuf)> {
    whisper_model_catalog()
        .iter()
        .copied()
        .filter(|spec| exclude_model_id.map_or(true, |exclude| exclude != spec.id))
        .filter_map(|spec| {
            let path = model_path_for_spec(models_dir, spec);
            if path.exists() {
                Some((spec, path))
            } else {
                None
            }
        })
        .max_by(|(a, _), (b, _)| {
            let a_key = (
                model_fit_level(*a, total_memory_gb),
                a.recommended_ram_gb,
                a.approx_size_gb.to_bits(),
            );
            let b_key = (
                model_fit_level(*b, total_memory_gb),
                b.recommended_ram_gb,
                b.approx_size_gb.to_bits(),
            );
            a_key.cmp(&b_key)
        })
}

fn resolve_active_model_path(
    config: &AppConfig,
    model_state: &LocalModelState,
) -> Result<PathBuf, String> {
    if let Some(path) = &config.whisper_model_path_override {
        return resolve_whisper_model_path(Some(path.as_str()));
    }

    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    let saved_path = settings
        .selected_model_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            "No local dictation model selected yet. Install one in onboarding before starting dictation."
                .to_string()
        })?;

    let path = PathBuf::from(saved_path);
    if !path.exists() {
        return Err(format!(
            "Selected dictation model file is missing: {}. Reinstall/select a model in onboarding.",
            path.display()
        ));
    }

    Ok(path)
}

fn download_whisper_model(model_spec: WhisperModelSpec, target_path: &Path) -> Result<(), String> {
    let target_str = target_path.to_string_lossy().to_string();
    let model_url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model_spec.file_name
    );

    #[cfg(target_os = "windows")]
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Invoke-WebRequest",
            "-Uri",
            &model_url,
            "-OutFile",
            &target_str,
        ])
        .output();

    #[cfg(not(target_os = "windows"))]
    let output = Command::new("curl")
        .args(["-L", "--fail", "--output", &target_str, &model_url])
        .output();

    match output {
        Ok(result) if result.status.success() && target_path.exists() => Ok(()),
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "no output".to_string()
            };
            Err(format!(
                "Could not download whisper model '{}' from {}: {}",
                model_spec.id, model_url, detail
            ))
        }
        Err(e) => Err(format!(
            "Could not start model download command. Install curl or PowerShell support and retry: {e}"
        )),
    }
}

fn build_onboarding_payload(
    config: &AppConfig,
    model_state: &LocalModelState,
) -> Result<DictationOnboardingPayload, String> {
    let device = build_device_profile();
    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    let override_model_path = config
        .whisper_model_path_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let override_model_exists = override_model_path
        .as_deref()
        .map(|value| Path::new(value).exists())
        .unwrap_or(false);

    let selected_model_path = if override_model_path.is_some() {
        override_model_path.clone()
    } else {
        settings.selected_model_path.clone()
    };
    let selected_model_exists = if override_model_path.is_some() {
        override_model_exists
    } else {
        settings
            .selected_model_path
            .as_deref()
            .map(|value| Path::new(value).exists())
            .unwrap_or(false)
    };
    let selected_model_id = if override_model_path.is_some() {
        Some("env-override".to_string())
    } else {
        settings.selected_model_id.clone()
    };
    let dictation_trigger = settings.dictation_trigger.as_deref().and_then(|value| {
        match normalize_dictation_trigger(value) {
            Ok(normalized) => Some(normalized),
            Err(error) => {
                log::warn!("Ignoring invalid persisted dictation trigger '{value}': {error}");
                None
            }
        }
    });
    let list_selected_model_id = if override_model_path.is_some() {
        None
    } else {
        settings.selected_model_id.as_deref()
    };
    let models = build_model_options(
        &model_state.models_dir,
        device.total_memory_gb,
        list_selected_model_id,
    );
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let detected_whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path);
    let whisper_cli_available = detected_whisper_cli_path.is_some();
    let onboarding_required = !selected_model_exists || !whisper_cli_available;

    Ok(DictationOnboardingPayload {
        onboarding_required,
        selected_model_id,
        selected_model_path,
        selected_model_exists,
        dictation_trigger,
        default_dictation_trigger: DEFAULT_DICTATION_TRIGGER.to_string(),
        whisper_cli_available,
        whisper_cli_path: detected_whisper_cli_path.unwrap_or(configured_whisper_cli_path),
        models_dir: model_state.models_dir.to_string_lossy().to_string(),
        device,
        models,
    })
}

#[tauri::command]
fn open_whisper_setup_page() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(WHISPER_CPP_SETUP_URL);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(WHISPER_CPP_SETUP_URL);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", WHISPER_CPP_SETUP_URL]);
        cmd
    };

    let status = command
        .status()
        .map_err(|e| format!("Failed to open download page: {e}"))?;
    if !status.success() {
        return Err(format!(
            "Could not open setup page automatically. Open {WHISPER_CPP_SETUP_URL} manually."
        ));
    }

    Ok(())
}

fn dominant_input_channel<T, F>(data: &[T], channels: usize, to_f32: &F) -> usize
where
    T: Copy,
    F: Fn(T) -> f32,
{
    if channels <= 1 {
        return 0;
    }

    let mut energy = vec![0.0_f32; channels];
    for frame in data.chunks(channels) {
        for (index, sample) in frame.iter().enumerate() {
            energy[index] += to_f32(*sample).abs();
        }
    }

    energy
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn push_downmixed<T, F>(data: &[T], channels: usize, target: &Arc<Mutex<Vec<f32>>>, to_f32: F)
where
    T: Copy,
    F: Fn(T) -> f32,
{
    if channels == 0 || data.is_empty() {
        return;
    }

    let dominant = dominant_input_channel(data, channels, &to_f32);
    let mut mono = Vec::with_capacity((data.len() / channels.max(1)).max(1));
    for frame in data.chunks(channels) {
        if frame.is_empty() {
            continue;
        }
        let picked = frame
            .get(dominant)
            .copied()
            .map(&to_f32)
            .unwrap_or_else(|| {
                let sum: f32 = frame.iter().copied().map(&to_f32).sum();
                sum / frame.len() as f32
            });
        mono.push(picked);
    }

    if let Ok(mut guard) = target.lock() {
        guard.extend(mono);
    }
}

fn sample_format_rank(sample_format: SampleFormat) -> u8 {
    match sample_format {
        SampleFormat::F32 => 3,
        SampleFormat::I16 => 2,
        SampleFormat::U16 => 1,
        _ => 0,
    }
}

fn choose_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, String> {
    if let Ok(default_config) = device.default_input_config() {
        return Ok(default_config);
    }

    let mut best: Option<(u8, u32, cpal::SupportedStreamConfig)> = None;
    let ranges = device
        .supported_input_configs()
        .map_err(|e| format!("Failed to query supported input configs: {e}"))?;

    for range in ranges {
        let candidate = range.with_max_sample_rate();
        let format_rank = sample_format_rank(candidate.sample_format());
        if format_rank == 0 {
            continue;
        }
        let candidate_rate = candidate.sample_rate().0;

        let replace = match &best {
            Some((best_rank, best_rate, _)) => {
                format_rank > *best_rank
                    || (format_rank == *best_rank && candidate_rate > *best_rate)
            }
            None => true,
        };
        if replace {
            best = Some((format_rank, candidate_rate, candidate));
        }
    }

    best.map(|(_, _, config)| config).ok_or_else(|| {
        "No compatible microphone input config found. Try a different input device.".to_string()
    })
}

fn create_input_stream_for_device(
    device: &cpal::Device,
    samples: Arc<Mutex<Vec<f32>>>,
) -> Result<(Stream, u32), String> {
    let supported_config = device
        .default_input_config()
        .or_else(|_| choose_input_config(device))
        .map_err(|e| format!("Failed to resolve input config: {e}"))?;
    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels() as usize;
    let config: cpal::StreamConfig = supported_config.clone().into();
    let err_fn = |err| {
        eprintln!("microphone stream error: {err}");
    };

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => {
            let sink = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        push_downmixed(data, channels, &sink, |v| v);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open f32 input stream: {e}"))?
        }
        SampleFormat::I16 => {
            let sink = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        push_downmixed(data, channels, &sink, |v| v as f32 / i16::MAX as f32);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open i16 input stream: {e}"))?
        }
        SampleFormat::U16 => {
            let sink = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        push_downmixed(data, channels, &sink, |v| {
                            (v as f32 / u16::MAX as f32) * 2.0 - 1.0
                        });
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open u16 input stream: {e}"))?
        }
        sample_format => {
            return Err(format!(
                "Unsupported input sample format: {sample_format:?}. Try a different input device."
            ));
        }
    };

    Ok((stream, sample_rate))
}

fn create_input_stream(samples: Arc<Mutex<Vec<f32>>>) -> Result<(Stream, u32), String> {
    let host = cpal::default_host();
    let mut candidate_devices: Vec<cpal::Device> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    if let Some(default_device) = host.default_input_device() {
        let name = default_device
            .name()
            .unwrap_or_else(|_| "default input".to_string());
        seen_names.insert(name);
        candidate_devices.push(default_device);
    }

    if let Ok(devices) = host.input_devices() {
        for device in devices {
            let name = device
                .name()
                .unwrap_or_else(|_| "unknown input".to_string());
            if seen_names.insert(name) {
                candidate_devices.push(device);
            }
        }
    }

    if candidate_devices.is_empty() {
        return Err(
            "No microphone input device found. In macOS Settings > Sound > Input, select a microphone and retry."
                .to_string(),
        );
    }

    let mut attempts: Vec<String> = Vec::new();
    for device in candidate_devices {
        let name = device
            .name()
            .unwrap_or_else(|_| "unknown input".to_string());

        match create_input_stream_for_device(&device, Arc::clone(&samples)) {
            Ok(result) => return Ok(result),
            Err(err) => attempts.push(format!("{name}: {err}")),
        }
    }

    Err(format!(
        "Could not open microphone input on this machine. Tried: {}. \
In macOS Settings > Privacy & Security > Microphone, allow this app/terminal, then pick an input device in Settings > Sound > Input and retry.",
        attempts.join(" | ")
    ))
}

fn spawn_recording_thread(
    samples: Arc<Mutex<Vec<f32>>>,
) -> Result<(mpsc::Sender<()>, thread::JoinHandle<()>, u32), String> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (init_tx, init_rx) = mpsc::channel::<Result<u32, String>>();
    let capture_samples = Arc::clone(&samples);

    let handle = thread::spawn(move || {
        let stream_result = create_input_stream(capture_samples);
        match stream_result {
            Ok((stream, sample_rate)) => match stream.play() {
                Ok(()) => {
                    let _ = init_tx.send(Ok(sample_rate));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = init_tx.send(Err(format!("Failed to start microphone stream: {e}")));
                }
            },
            Err(e) => {
                let _ = init_tx.send(Err(e));
            }
        }
    });

    let sample_rate = match init_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(rate)) => rate,
        Ok(Err(e)) => {
            let _ = handle.join();
            return Err(e);
        }
        Err(_) => {
            let _ = stop_tx.send(());
            let _ = handle.join();
            return Err("Timed out while opening microphone stream.".to_string());
        }
    };

    Ok((stop_tx, handle, sample_rate))
}

fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = target_rate as f32 / source_rate as f32;
    let out_len = ((samples.len() as f32) * ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f32 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = src_pos - idx as f32;

        let a = samples.get(idx).copied().unwrap_or(0.0);
        let b = samples.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }

    out
}

fn audio_duration_seconds(samples: &[f32], sample_rate: u32) -> f32 {
    if sample_rate == 0 {
        return 0.0;
    }
    samples.len() as f32 / sample_rate as f32
}

fn audio_peak_and_rms(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }

    let mut peak = 0.0_f32;
    let mut energy_sum = 0.0_f32;
    for sample in samples {
        let value = sample.abs();
        peak = peak.max(value);
        energy_sum += sample * sample;
    }
    let rms = (energy_sum / samples.len() as f32).sqrt();
    (peak, rms)
}

fn remove_dc_offset(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    let mean = samples.iter().copied().sum::<f32>() / samples.len() as f32;
    if mean.abs() < 1e-6 {
        return;
    }
    for sample in samples {
        *sample -= mean;
    }
}

fn trim_silence_edges(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 {
        return Vec::new();
    }

    let window = (((sample_rate as u64) * SILENCE_WINDOW_MS as u64) / 1000).max(1) as usize;
    let mut first_speech: Option<usize> = None;
    let mut last_speech_end: Option<usize> = None;

    for (window_index, chunk) in samples.chunks(window).enumerate() {
        if chunk.is_empty() {
            continue;
        }
        let mean_abs = chunk.iter().map(|sample| sample.abs()).sum::<f32>() / chunk.len() as f32;
        if mean_abs >= SILENCE_GATE_ABS_MEAN {
            let start = window_index * window;
            let end = (start + chunk.len()).min(samples.len());
            if first_speech.is_none() {
                first_speech = Some(start);
            }
            last_speech_end = Some(end);
        }
    }

    let (start, end) = match (first_speech, last_speech_end) {
        (Some(start), Some(end)) if end > start => (start, end),
        _ => return samples.to_vec(),
    };

    let pad = (((sample_rate as u64) * SILENCE_TRIM_PAD_MS as u64) / 1000) as usize;
    let bounded_start = start.saturating_sub(pad);
    let bounded_end = (end + pad).min(samples.len());
    samples[bounded_start..bounded_end].to_vec()
}

fn normalize_audio_gain(samples: &mut [f32]) {
    let (peak, rms) = audio_peak_and_rms(samples);
    if peak <= 0.0 || rms < MIN_SPEECH_RMS {
        return;
    }

    let mut gain = 1.0_f32;
    if peak < 0.35 {
        gain = (0.85 / peak).clamp(1.0, 8.0);
    } else if peak > 0.98 {
        gain = (0.98 / peak).clamp(0.1, 1.0);
    }

    if (gain - 1.0).abs() < 0.05 {
        return;
    }

    for sample in samples {
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
}

fn sanitize_audio_for_transcription(samples: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    let mut prepared = if sample_rate == WHISPER_SAMPLE_RATE {
        samples
    } else {
        resample_linear(&samples, sample_rate, WHISPER_SAMPLE_RATE)
    };

    remove_dc_offset(&mut prepared);
    prepared = trim_silence_edges(&prepared, WHISPER_SAMPLE_RATE);
    normalize_audio_gain(&mut prepared);
    prepared
}

fn write_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create wav file {}: {e}", path.display()))?;
    for sample in samples {
        let clipped = sample.clamp(-1.0, 1.0);
        let pcm = (clipped * i16::MAX as f32) as i16;
        writer
            .write_sample(pcm)
            .map_err(|e| format!("Failed to write wav sample: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize wav file: {e}"))?;

    Ok(())
}

fn is_transcript_artifact_token(token: &str) -> bool {
    let normalized = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
    let upper = normalized.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "BLANK_AUDIO" | "NOISE" | "MUSIC" | "SILENCE"
    )
}

fn normalize_transcript_text(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|token| !is_transcript_artifact_token(token))
        .collect::<Vec<_>>()
        .join(" ")
}

fn recommended_whisper_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .clamp(2, 8)
}

fn run_whisper_cli(
    whisper_cli_path: &str,
    model_path: &Path,
    wav_path: &Path,
    out_prefix: &Path,
    decode_profile: WhisperDecodeProfile,
) -> Result<(), String> {
    let threads = recommended_whisper_threads().to_string();
    let beam = decode_profile.beam_size.to_string();
    let best_of = decode_profile.best_of.to_string();
    let output = Command::new(whisper_cli_path)
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(wav_path)
        .arg("-l")
        .arg("en")
        .arg("-t")
        .arg(&threads)
        .arg("-bs")
        .arg(&beam)
        .arg("-bo")
        .arg(&best_of)
        .arg("-otxt")
        .arg("-nt")
        .arg("-np")
        .arg("-of")
        .arg(out_prefix)
        .output()
        .map_err(|e| {
            format!(
                "Failed to execute whisper cli '{whisper_cli_path}': {e}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "no error output".to_string()
    };
    Err(format!("whisper-cli transcription failed: {detail}"))
}

fn read_clean_transcript(path: &Path) -> Result<String, String> {
    let transcript = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "whisper-cli ran but transcript file is missing at {}: {e}",
            path.display()
        )
    })?;
    Ok(normalize_transcript_text(&transcript))
}

fn normalized_token_for_quality(token: &str) -> String {
    token
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn transcript_looks_low_confidence(cleaned: &str, audio_seconds: f32) -> bool {
    let tokens = cleaned
        .split_whitespace()
        .map(normalized_token_for_quality)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        return true;
    }

    let unique = tokens.iter().collect::<HashSet<_>>();
    if audio_seconds >= LOW_CONFIDENCE_RETRY_SECONDS && tokens.len() <= 2 {
        return true;
    }
    if tokens.len() >= 2 && unique.len() == 1 {
        return true;
    }
    if audio_seconds >= 8.0 {
        let unique_ratio = unique.len() as f32 / tokens.len() as f32;
        if unique_ratio < 0.35 {
            return true;
        }
        if cleaned.chars().count() < 16 {
            return true;
        }
    }

    false
}

fn transcript_information_score(cleaned: &str) -> usize {
    let tokens = cleaned
        .split_whitespace()
        .map(normalized_token_for_quality)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let unique = tokens.iter().collect::<HashSet<_>>();
    let char_count = cleaned.chars().count();
    (unique.len() * 6) + (tokens.len() * 3) + char_count.min(160)
}

fn transcribe_samples(
    model_path: PathBuf,
    whisper_cli_path: String,
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<String, String> {
    let prepared = sanitize_audio_for_transcription(samples, sample_rate);

    if prepared.is_empty() {
        return Err("No audio captured. Check microphone input and try again.".to_string());
    }
    let (peak, rms) = audio_peak_and_rms(&prepared);
    if peak < MIN_SPEECH_PEAK || rms < MIN_SPEECH_RMS {
        return Err("No speech detected in the recorded audio.".to_string());
    }
    let audio_seconds = audio_duration_seconds(&prepared, WHISPER_SAMPLE_RATE);
    if audio_seconds < MIN_TRANSCRIBE_SECONDS {
        return Err(
            "Captured audio was too short. Hold fn (or record) a bit longer and retry.".to_string(),
        );
    }

    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir();
    let base_name = format!("dicktaint-{}-{tick}", std::process::id());
    let wav_path = temp_dir.join(format!("{base_name}.wav"));
    let out_prefix = temp_dir.join(format!("{base_name}-transcript"));
    let txt_path = out_prefix.with_extension("txt");

    write_wav(&wav_path, &prepared, WHISPER_SAMPLE_RATE)?;

    if let Err(error) = run_whisper_cli(
        &whisper_cli_path,
        &model_path,
        &wav_path,
        &out_prefix,
        FAST_DECODE_PROFILE,
    ) {
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_file(&txt_path);
        return Err(error);
    }

    let mut cleaned = match read_clean_transcript(&txt_path) {
        Ok(value) => value,
        Err(error) => {
            let _ = std::fs::remove_file(&wav_path);
            let _ = std::fs::remove_file(&txt_path);
            return Err(error);
        }
    };
    if transcript_looks_low_confidence(&cleaned, audio_seconds) {
        let retry = run_whisper_cli(
            &whisper_cli_path,
            &model_path,
            &wav_path,
            &out_prefix,
            ACCURATE_DECODE_PROFILE,
        )
        .and_then(|_| read_clean_transcript(&txt_path))
        .unwrap_or_default();
        if transcript_information_score(&retry) > transcript_information_score(&cleaned) {
            cleaned = retry;
        }
    }

    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(&txt_path);

    if cleaned.is_empty() {
        return Err("No speech detected in the recorded audio.".to_string());
    }

    Ok(cleaned)
}

#[tauri::command]
fn get_dictation_onboarding(
    app: tauri::AppHandle,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationOnboardingPayload, String> {
    let payload = build_onboarding_payload(config.inner(), model_state.inner())?;
    if let Err(error) = apply_registered_hotkey(
        &app,
        hotkey_state.inner(),
        payload.dictation_trigger.as_deref(),
    ) {
        log::warn!("get_dictation_onboarding: failed to apply global hotkey: {error}");
    }
    Ok(payload)
}

#[tauri::command]
fn get_dictation_trigger(
    model_state: State<'_, LocalModelState>,
) -> Result<DictationTriggerPayload, String> {
    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    Ok(dictation_trigger_payload(&settings))
}

#[tauri::command]
fn set_dictation_trigger(
    app: tauri::AppHandle,
    trigger: String,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let normalized = normalize_dictation_trigger(&trigger)?;
    let previous_trigger = {
        let settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        settings.dictation_trigger.clone()
    };

    apply_registered_hotkey(&app, hotkey_state.inner(), Some(&normalized))?;

    let settings_path = model_state.settings_path.clone();
    let rollback_trigger = previous_trigger
        .as_deref()
        .and_then(|value| normalize_dictation_trigger(value).ok());
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    settings.dictation_trigger = Some(normalized.clone());
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        if let Err(restore_error) =
            apply_registered_hotkey(&app, hotkey_state.inner(), rollback_trigger.as_deref())
        {
            log::warn!("set_dictation_trigger: failed to restore previous hotkey after save error: {restore_error}");
        }
        return Err(error);
    }
    Ok(dictation_trigger_payload(&settings))
}

#[tauri::command]
fn clear_dictation_trigger(
    app: tauri::AppHandle,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let previous_trigger = {
        let settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        settings.dictation_trigger.clone()
    };

    apply_registered_hotkey(&app, hotkey_state.inner(), None)?;

    let settings_path = model_state.settings_path.clone();
    let rollback_trigger = previous_trigger
        .as_deref()
        .and_then(|value| normalize_dictation_trigger(value).ok());
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    settings.dictation_trigger = None;
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        if let Err(restore_error) =
            apply_registered_hotkey(&app, hotkey_state.inner(), rollback_trigger.as_deref())
        {
            log::warn!(
                "clear_dictation_trigger: failed to restore previous hotkey after save error: {restore_error}"
            );
        }
        return Err(error);
    }
    Ok(dictation_trigger_payload(&settings))
}

#[tauri::command]
async fn install_dictation_model(
    model: String,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<DictationModelSelection, String> {
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);
    ensure_whisper_cli_available(&whisper_cli_path)?;
    let trimmed_id = model.trim();
    if trimmed_id.is_empty() {
        return Err("Missing model id".to_string());
    }

    let model_spec = find_whisper_model_spec(trimmed_id).ok_or_else(|| {
        let ids = whisper_model_catalog()
            .iter()
            .map(|spec| spec.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("Unsupported dictation model '{trimmed_id}'. Available models: {ids}")
    })?;
    let models_dir = model_state.models_dir.clone();
    let settings_path = model_state.settings_path.clone();
    let settings = Arc::clone(&model_state.settings);

    let install_task =
        tauri::async_runtime::spawn_blocking(move || -> Result<DictationModelSelection, String> {
            fs::create_dir_all(&models_dir).map_err(|e| {
                format!(
                    "Failed to create model directory {}: {e}",
                    models_dir.display()
                )
            })?;

            let target_path = model_path_for_spec(&models_dir, model_spec);
            if !target_path.exists() {
                download_whisper_model(model_spec, &target_path)?;
                if !target_path.exists() {
                    return Err(format!(
                        "Model download completed but file is still missing at {}.",
                        target_path.display()
                    ));
                }
            }

            let selected_model_path = target_path.to_string_lossy().to_string();
            {
                let mut settings = settings
                    .lock()
                    .map_err(|_| "Failed to lock local model settings".to_string())?;
                settings.selected_model_id = Some(model_spec.id.to_string());
                settings.selected_model_path = Some(selected_model_path.clone());
                save_local_settings(&settings_path, &settings)?;
            }

            Ok(DictationModelSelection {
                selected_model_id: model_spec.id.to_string(),
                selected_model_path,
                installed: true,
            })
        });

    install_task
        .await
        .map_err(|e| format!("Model install task failed: {e}"))?
}

#[tauri::command]
async fn delete_dictation_model(
    model: String,
    model_state: State<'_, LocalModelState>,
) -> Result<DictationModelDeletion, String> {
    let trimmed_id = model.trim();
    if trimmed_id.is_empty() {
        return Err("Missing model id".to_string());
    }

    let model_spec = find_whisper_model_spec(trimmed_id).ok_or_else(|| {
        let ids = whisper_model_catalog()
            .iter()
            .map(|spec| spec.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("Unsupported dictation model '{trimmed_id}'. Available models: {ids}")
    })?;

    let models_dir = model_state.models_dir.clone();
    let settings_path = model_state.settings_path.clone();
    let settings = Arc::clone(&model_state.settings);
    let total_memory_gb = system_memory_gb();

    let delete_task =
        tauri::async_runtime::spawn_blocking(move || -> Result<DictationModelDeletion, String> {
            let target_path = model_path_for_spec(&models_dir, model_spec);
            if let Err(e) = fs::remove_file(&target_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!(
                        "Failed to delete model '{}' at {}: {e}",
                        model_spec.id,
                        target_path.display()
                    ));
                }
            }

            let target_path_string = target_path.to_string_lossy().to_string();
            let (selected_model_id, selected_model_path) = {
                let mut settings = settings
                    .lock()
                    .map_err(|_| "Failed to lock local model settings".to_string())?;

                let deleted_selected_model = settings.selected_model_id.as_deref()
                    == Some(model_spec.id)
                    || settings
                        .selected_model_path
                        .as_deref()
                        .is_some_and(|path| path == target_path_string);

                if deleted_selected_model {
                    if let Some((fallback_spec, fallback_path)) =
                        pick_best_installed_model(&models_dir, total_memory_gb, Some(model_spec.id))
                    {
                        settings.selected_model_id = Some(fallback_spec.id.to_string());
                        settings.selected_model_path =
                            Some(fallback_path.to_string_lossy().to_string());
                    } else {
                        settings.selected_model_id = None;
                        settings.selected_model_path = None;
                    }
                    save_local_settings(&settings_path, &settings)?;
                }

                (
                    settings.selected_model_id.clone(),
                    settings.selected_model_path.clone(),
                )
            };

            Ok(DictationModelDeletion {
                deleted_model_id: model_spec.id.to_string(),
                selected_model_id,
                selected_model_path,
            })
        });

    delete_task
        .await
        .map_err(|e| format!("Model delete task failed: {e}"))?
}

#[tauri::command]
fn start_native_dictation(
    app: tauri::AppHandle,
    dictation: State<'_, DictationState>,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<(), String> {
    resolve_active_model_path(config.inner(), model_state.inner())?;
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);
    ensure_whisper_cli_available(&whisper_cli_path)?;

    let mut guard = dictation
        .inner()
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())?;
    if guard.is_some() {
        return Err("Dictation already running.".to_string());
    }

    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let (stop_tx, thread_handle, sample_rate) = spawn_recording_thread(Arc::clone(&samples))?;
    *guard = Some(ActiveRecording {
        stop_tx,
        thread_handle,
        samples,
        sample_rate,
    });
    app.emit(
        DICTATION_STATE_EVENT,
        DictationStatePayload {
            state: "listening".into(),
            error: None,
            transcript: None,
        },
    )
    .ok();
    Ok(())
}

#[tauri::command]
async fn stop_native_dictation(
    app: tauri::AppHandle,
    dictation: State<'_, DictationState>,
    config: State<'_, AppConfig>,
    model_state: State<'_, LocalModelState>,
) -> Result<String, String> {
    let recording = {
        let mut guard = dictation
            .inner()
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard
            .take()
            .ok_or_else(|| "Dictation is not running.".to_string())?
    };

    let _ = recording.stop_tx.send(());
    if recording.thread_handle.join().is_err() {
        app.emit(
            DICTATION_STATE_EVENT,
            DictationStatePayload {
                state: "error".into(),
                error: Some("Audio capture thread crashed.".into()),
                transcript: None,
            },
        )
        .ok();
        return Err("Audio capture thread crashed.".to_string());
    }

    let captured_samples = {
        let mut guard = recording
            .samples
            .lock()
            .map_err(|_| "Failed to read captured audio".to_string())?;
        std::mem::take(&mut *guard)
    };
    let model_path = resolve_active_model_path(config.inner(), model_state.inner())?;
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);

    app.emit(
        DICTATION_STATE_EVENT,
        DictationStatePayload {
            state: "processing".into(),
            error: None,
            transcript: None,
        },
    )
    .ok();

    let result = tauri::async_runtime::spawn_blocking(move || {
        transcribe_samples(
            model_path,
            whisper_cli_path,
            captured_samples,
            recording.sample_rate,
        )
    })
    .await
    .map_err(|e| {
        app.emit(
            DICTATION_STATE_EVENT,
            DictationStatePayload {
                state: "error".into(),
                error: Some(e.to_string()),
                transcript: None,
            },
        )
        .ok();
        format!("Failed to run transcription task: {e}")
    })?;

    match result {
        Ok(transcript) => {
            app.emit(
                DICTATION_STATE_EVENT,
                DictationStatePayload {
                    state: "idle".into(),
                    error: None,
                    transcript: Some(transcript.clone()),
                },
            )
            .ok();
            Ok(transcript)
        }
        Err(e) => {
            app.emit(
                DICTATION_STATE_EVENT,
                DictationStatePayload {
                    state: "error".into(),
                    error: Some(e.clone()),
                    transcript: None,
                },
            )
            .ok();
            Err(e)
        }
    }
}

#[tauri::command]
fn cancel_native_dictation(
    app: tauri::AppHandle,
    dictation: State<'_, DictationState>,
) -> Result<(), String> {
    let recording = {
        let mut guard = dictation
            .inner()
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard.take()
    };

    if let Some(recording) = recording {
        let _ = recording.stop_tx.send(());
        let _ = recording.thread_handle.join();
    }

    app.emit(
        DICTATION_STATE_EVENT,
        DictationStatePayload {
            state: "idle".into(),
            error: None,
            transcript: None,
        },
    )
    .ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{normalize_dictation_trigger, resample_linear, whisper_help_text_looks_valid};

    #[test]
    fn resample_linear_returns_same_when_rate_matches() {
        let source = vec![0.0_f32, 0.5, -0.5, 1.0];
        let out = resample_linear(&source, 16_000, 16_000);
        assert_eq!(out, source);
    }

    #[test]
    fn resample_linear_produces_output_when_rate_changes() {
        let source = vec![0.0_f32, 1.0, 0.0, -1.0];
        let out = resample_linear(&source, 8_000, 16_000);
        assert!(out.len() > source.len());
        assert!(out.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn whisper_help_text_accepts_real_help_snippet() {
        let stdout = "usage: whisper-cli [options] file0.wav\n  -m FNAME  model path";
        assert!(whisper_help_text_looks_valid(stdout, ""));
    }

    #[test]
    fn whisper_help_text_rejects_placeholder_snippet() {
        let stderr =
            "Bundled whisper-cli placeholder. Replace with a real whisper-cli sidecar binary.";
        assert!(!whisper_help_text_looks_valid("", stderr));
    }

    #[test]
    fn normalize_dictation_trigger_accepts_valid_combo() {
        assert_eq!(
            normalize_dictation_trigger("cmdorctrl + shift + d").unwrap(),
            "CmdOrCtrl+Shift+D".to_string()
        );
    }

    #[test]
    fn normalize_dictation_trigger_accepts_fn_key() {
        assert_eq!(normalize_dictation_trigger("fn").unwrap(), "Fn".to_string());
        assert_eq!(normalize_dictation_trigger("globe").unwrap(), "Fn".to_string());
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
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init();

    let whisper_model_path_override = std::env::var("WHISPER_MODEL_PATH").ok();
    let whisper_cli_path_override = std::env::var("WHISPER_CLI_PATH").ok();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Err(error) = app.emit(DICTATION_HOTKEY_EVENT, ()) {
                            log::warn!("Failed to emit dictation hotkey event: {error}");
                        }
                    }
                })
                .build(),
        )
        .setup(move |app| {
            let bundled_whisper_cli_path = resolve_bundled_whisper_cli_path(app.handle());
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                format!(
                    "Failed to resolve Tauri app data directory while initializing local dictation paths: {e}"
                )
            })?;
            let (models_dir, settings_path) = resolve_local_paths(&app_data_dir).map_err(|e| {
                format!(
                    "Failed to initialize local dictation model paths under {}: {e}",
                    app_data_dir.display()
                )
            })?;
            let initial_settings = load_local_settings(&settings_path);
            let initial_dictation_trigger = initial_settings.dictation_trigger.clone();

            app.manage(AppConfig {
                whisper_model_path_override: whisper_model_path_override.clone(),
                whisper_cli_path_override: whisper_cli_path_override.clone(),
                bundled_whisper_cli_path,
            });
            app.manage(LocalModelState {
                settings_path,
                models_dir,
                settings: Arc::new(Mutex::new(initial_settings)),
            });
            app.manage(DictationState::default());
            app.manage(GlobalHotkeyState::default());

            if let Err(error) = apply_registered_hotkey(
                app.handle(),
                app.state::<GlobalHotkeyState>().inner(),
                initial_dictation_trigger.as_deref(),
            ) {
                log::warn!("Failed to apply initial global hotkey: {error}");
            }

            // Create the always-on-top floating pill window
            {
                use tauri::WebviewWindowBuilder;
                let pill_w = 110.0_f64;
                let pill_h = 34.0_f64;
                let (px, py) = app.primary_monitor().ok().flatten()
                    .map(|m| {
                        let s = m.scale_factor();
                        let sz = m.size();
                        let pos = m.position();
                        let x = pos.x as f64 + (sz.width as f64 / s - pill_w) / 2.0;
                        let y = pos.y as f64 + sz.height as f64 / s - pill_h - 40.0;
                        (x as i32, y as i32)
                    })
                    .unwrap_or((800, 960));

                if let Err(e) = WebviewWindowBuilder::new(app, "pill", tauri::WebviewUrl::App("pill.html".into()))
                    .title("")
                    .inner_size(pill_w, pill_h)
                    .position(px as f64, py as f64)
                    .decorations(false)
                    .transparent(true)
                    .background_color(tauri::webview::Color(0, 0, 0, 0))
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .resizable(false)
                    .focused(false)
                    .shadow(false)
                    .build()
                {
                    log::warn!("Failed to create pill window: {e}. App will continue without it.");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_dictation_onboarding,
            get_dictation_trigger,
            set_dictation_trigger,
            clear_dictation_trigger,
            open_whisper_setup_page,
            install_dictation_model,
            delete_dictation_model,
            start_native_dictation,
            stop_native_dictation,
            cancel_native_dictation
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            show_main_window(app_handle);
        }
    });
}
