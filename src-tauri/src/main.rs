#[cfg(target_os = "macos")]
use block2::RcBlock;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
#[cfg(target_os = "macos")]
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::str::FromStr;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, AtomicPtr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, State};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
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
const DICTATION_STATE_EVENT: &str = "dictation:state-changed";
const DICTATION_AUDIO_LEVEL_EVENT: &str = "dictation:audio-level";
const PILL_STATUS_EVENT: &str = "dicktaint://pill-status";
const WHISPER_CPP_SETUP_URL: &str = "https://github.com/ggml-org/whisper.cpp#quick-start";
const START_HIDDEN_ENV: &str = "DICKTAINT_START_HIDDEN";
const PILL_WINDOW_LABEL_PREFIX: &str = "pill";
const PILL_WINDOW_BASE_WIDTH: f64 = 108.0;
const PILL_WINDOW_MIN_WIDTH: f64 = 92.0;
const PILL_WINDOW_HEIGHT: f64 = 26.0;
const PILL_WINDOW_BOTTOM_MARGIN: i32 = 14;
const MAX_PILL_WINDOWS: usize = 6;
const MIN_TRANSCRIPTION_AUDIO_PEAK: f32 = 0.008;
const MIN_TRANSCRIPTION_AUDIO_RMS: f32 = 0.0008;
const TARGET_TRANSCRIPTION_AUDIO_PEAK: f32 = 0.85;
const MAX_TRANSCRIPTION_AUDIO_GAIN: f32 = 16.0;
const LIVE_AUDIO_BAR_COUNT: usize = 12;
const LIVE_AUDIO_EMIT_INTERVAL_MS: u64 = 45;
const INPUT_STREAM_PROBE_TIMEOUT_MS: u64 = 1_500;
const INPUT_STREAM_PROBE_POLL_INTERVAL_MS: u64 = 40;
const INPUT_STREAM_PROBE_MIN_DURATION_MS: u32 = 120;

#[derive(Clone, Serialize)]
struct DictationStatePayload {
    state: String,
    error: Option<String>,
    transcript: Option<String>,
    session_id: Option<u64>,
}

#[derive(Clone, Serialize)]
struct DictationAudioLevelPayload {
    session_id: u64,
    peak_abs: f32,
    rms: f32,
    level: f32,
    bars: Vec<f32>,
}

#[derive(Clone, Serialize)]
struct PillStatusPayload {
    message: String,
    state: String,
    visible: bool,
}

#[derive(Clone)]
struct AppConfig {
    whisper_model_path_override: Option<String>,
    whisper_cli_path_override: Option<String>,
    bundled_whisper_cli_path: Option<String>,
}

struct DictationState {
    active_recording: Mutex<Option<ActiveRecording>>,
    next_session_id: AtomicU64,
}

impl Default for DictationState {
    fn default() -> Self {
        Self {
            active_recording: Mutex::new(None),
            next_session_id: AtomicU64::new(1),
        }
    }
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
    preferred_input_device: Option<String>,
    dictation_trigger: Option<String>,
    dictation_trigger_enabled: Option<bool>,
    focused_field_insert_enabled: Option<bool>,
}

struct LocalModelState {
    settings_path: PathBuf,
    models_dir: PathBuf,
    settings: Arc<Mutex<LocalSettings>>,
}

#[derive(Clone, Default)]
enum HotkeyDeliveryMode {
    #[default]
    Disabled,
    GlobalToggle,
    GlobalHold,
    FocusedWindowHold,
}

impl HotkeyDeliveryMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::GlobalToggle => "global-toggle",
            Self::GlobalHold => "global-hold",
            Self::FocusedWindowHold => "focused-window-hold",
        }
    }
}

#[derive(Clone)]
struct TriggerRuntimeDetails {
    mode: HotkeyDeliveryMode,
    status: String,
    permission_hint: Option<String>,
}

#[derive(Default)]
struct GlobalHotkeyState {
    registered_trigger: Mutex<Option<String>>,
    runtime_details: Mutex<TriggerRuntimeDetails>,
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
const CG_EVENT_TAP_LOCATION_HID: u32 = 0;
#[cfg(target_os = "macos")]
const CG_EVENT_TAP_PLACEMENT_HEAD_INSERT: u32 = 0;
#[cfg(target_os = "macos")]
const CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
#[cfg(target_os = "macos")]
const CG_EVENT_TYPE_FLAGS_CHANGED: u32 = 12;
#[cfg(target_os = "macos")]
const CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
#[cfg(target_os = "macos")]
const CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

#[cfg(target_os = "macos")]
const MACOS_COMMAND_FLAG_MASK: CGEventFlags = 1 << 20;
#[cfg(target_os = "macos")]
const MACOS_FN_FLAG_MASK: CGEventFlags = 1 << 23;
#[cfg(target_os = "macos")]
const MACOS_NON_FN_MODIFIER_MASK: CGEventFlags = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20);
#[cfg(target_os = "macos")]
const KEYCODE_COMMAND: u16 = 0x37;
#[cfg(target_os = "macos")]
const KEYCODE_V: u16 = 0x09;

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
    fn CGEventCreateKeyboardEvent(
        source: *const c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
    fn CGEventPost(tap: u32, event: CGEventRef);
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
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[cfg(target_os = "macos")]
struct MacFnCallbackContext {
    app: tauri::AppHandle,
    enabled: AtomicBool,
    fn_down: AtomicBool,
    tap: AtomicPtr<c_void>,
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
    if user_info.is_null() {
        return event;
    }

    let callback_ctx = &*(user_info as *const MacFnCallbackContext);
    if event_type == CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT
        || event_type == CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT
    {
        let tap = callback_ctx.tap.load(Ordering::Relaxed);
        if !tap.is_null() {
            CGEventTapEnable(tap.cast::<c_void>(), true);
        }
        callback_ctx.fn_down.store(false, Ordering::Relaxed);
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
    available_input_devices: Vec<DictationInputDevice>,
    preferred_input_device: Option<String>,
    dictation_trigger: Option<String>,
    default_dictation_trigger: String,
    dictation_trigger_mode: String,
    dictation_trigger_status: String,
    dictation_trigger_permission_hint: Option<String>,
    focused_field_insert_enabled: bool,
    focused_field_insert_permission_granted: bool,
    focused_field_insert_permission_status: String,
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
    trigger_mode: String,
    trigger_status: String,
    trigger_permission_hint: Option<String>,
}

#[derive(Serialize)]
struct FocusedFieldInsertPayload {
    enabled: bool,
    permission_granted: bool,
    permission_status: String,
}

#[derive(Clone)]
struct FocusedFieldInsertPermissionStatus {
    granted: bool,
    status: String,
}

#[derive(Clone, Serialize)]
struct DictationInputDevice {
    name: String,
    is_default: bool,
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

#[derive(Clone, Copy, Debug)]
struct AudioSignalStats {
    peak_abs: f32,
    rms: f32,
    duration_secs: f32,
}

#[derive(Clone)]
struct LiveAudioMeter {
    app: tauri::AppHandle,
    session_id: u64,
    last_emitted_at: Arc<Mutex<Option<Instant>>>,
}

struct ActiveRecording {
    session_id: u64,
    input_device_name: String,
    stop_tx: mpsc::Sender<()>,
    thread_handle: thread::JoinHandle<()>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

#[derive(Clone, Copy)]
enum BackendHotkeyAction {
    Toggle,
    HoldStart,
    HoldStop,
}

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

fn sync_pill_after_main_window_hide(app: &tauri::AppHandle) {
    let state = if dictation_is_running(app).unwrap_or(false) {
        "listening"
    } else {
        "idle"
    };
    sync_pill_for_dictation_state(app, state, None);
}

fn active_hotkey_label(app: &tauri::AppHandle) -> String {
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

fn idle_pill_message(app: &tauri::AppHandle) -> String {
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

fn emit_pill_status(
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

fn sync_pill_for_dictation_state(app: &tauri::AppHandle, state: &str, error: Option<&str>) {
    let hotkey_state = app.state::<GlobalHotkeyState>();
    let runtime = current_trigger_runtime_details(hotkey_state.inner()).unwrap_or_default();
    let label = active_hotkey_label(app);

    let (message, pill_state) = match state {
        "listening" => {
            let message = match runtime.mode {
                HotkeyDeliveryMode::GlobalHold | HotkeyDeliveryMode::FocusedWindowHold => {
                    format!("Listening - release {label}")
                }
                HotkeyDeliveryMode::GlobalToggle => format!("Listening - press {label} again"),
                HotkeyDeliveryMode::Disabled => "Listening...".to_string(),
            };
            (message, "live")
        }
        "processing" => ("Transcribing...".to_string(), "working"),
        "error" => (
            if error
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
            {
                "Dictation error - check status".to_string()
            } else {
                idle_pill_message(app)
            },
            if error
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
            {
                "error"
            } else {
                "idle"
            },
        ),
        _ => (idle_pill_message(app), "idle"),
    };

    emit_pill_status(app, message, pill_state, true);
}

fn emit_dictation_state(
    app: &tauri::AppHandle,
    state: &str,
    error: Option<String>,
    transcript: Option<String>,
    session_id: Option<u64>,
) {
    sync_pill_for_dictation_state(app, state, error.as_deref());
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

fn current_active_session_id(app: &tauri::AppHandle) -> Result<Option<u64>, String> {
    let dictation = app.state::<DictationState>();
    dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())
        .map(|guard| guard.as_ref().map(|recording| recording.session_id))
}

fn dictation_is_running(app: &tauri::AppHandle) -> Result<bool, String> {
    current_active_session_id(app).map(|value| value.is_some())
}

fn dispatch_backend_hotkey_action(app: &tauri::AppHandle, action: BackendHotkeyAction) {
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
fn pill_window_width_for_monitor(monitor: &tauri::Monitor) -> f64 {
    let clamped_scale = monitor.scale_factor().clamp(1.0, 2.0);
    PILL_WINDOW_MIN_WIDTH + (clamped_scale - 1.0) * (PILL_WINDOW_BASE_WIDTH - PILL_WINDOW_MIN_WIDTH)
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

fn default_dictation_trigger() -> String {
    normalize_dictation_trigger(DEFAULT_DICTATION_TRIGGER)
        .unwrap_or_else(|_| DEFAULT_DICTATION_TRIGGER.to_string())
}

fn resolve_effective_dictation_trigger(settings: &LocalSettings) -> Option<String> {
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

fn focused_field_insert_enabled(settings: &LocalSettings) -> bool {
    matches!(settings.focused_field_insert_enabled, Some(true))
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

fn focused_field_insert_permission_status(
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

impl Default for TriggerRuntimeDetails {
    fn default() -> Self {
        Self {
            mode: HotkeyDeliveryMode::Disabled,
            status: "Hotkey disabled.".to_string(),
            permission_hint: None,
        }
    }
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

fn current_trigger_runtime_details(
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

fn onboarding_runtime_details(
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

fn dictation_trigger_payload(
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

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn apply_registered_hotkey(
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
fn apply_registered_hotkey(
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

fn preferred_arch_variants() -> Vec<&'static str> {
    let primary = std::env::consts::ARCH;
    let mut variants = vec![primary];

    #[cfg(target_os = "macos")]
    {
        for fallback in ["aarch64", "x86_64"] {
            if !variants.contains(&fallback) {
                variants.push(fallback);
            }
        }
    }

    variants
}

fn preferred_whisper_cli_names() -> Vec<String> {
    let os = std::env::consts::OS;
    let mut names = Vec::<String>::new();

    if os == "windows" {
        for arch in preferred_arch_variants() {
            names.push(format!("whisper-cli-{arch}-pc-windows-msvc.exe"));
        }
        names.push("whisper-cli.exe".to_string());
    } else if os == "macos" {
        for arch in preferred_arch_variants() {
            names.push(format!("whisper-cli-{arch}-apple-darwin"));
        }
        names.push("whisper-cli".to_string());
    } else if os == "linux" {
        for arch in preferred_arch_variants() {
            names.push(format!("whisper-cli-{arch}-unknown-linux-gnu"));
        }
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
    whisper_model_catalog()
        .iter()
        .copied()
        .filter(|spec| model_fit_level(*spec, total_memory_gb) > 0)
        .max_by(|a, b| {
            // Prefer strongest runnable model for the machine, not merely the smallest.
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
        .filter(|spec| !exclude_model_id.is_some_and(|exclude| exclude == spec.id))
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
    hotkey_state: &GlobalHotkeyState,
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
    let dictation_trigger = resolve_effective_dictation_trigger(&settings);
    let registered_trigger = current_registered_hotkey(hotkey_state).ok().flatten();
    let registered_runtime = current_trigger_runtime_details(hotkey_state).ok();
    let trigger_runtime = onboarding_runtime_details(
        dictation_trigger.as_deref(),
        registered_trigger.as_deref(),
        registered_runtime.as_ref(),
    );
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
    let focused_field_permission =
        focused_field_insert_permission_status(focused_field_insert_enabled(&settings), false);
    let available_input_devices = list_input_devices();

    Ok(DictationOnboardingPayload {
        onboarding_required,
        selected_model_id,
        selected_model_path,
        selected_model_exists,
        available_input_devices,
        preferred_input_device: settings.preferred_input_device.clone(),
        dictation_trigger,
        default_dictation_trigger: default_dictation_trigger(),
        dictation_trigger_mode: trigger_runtime.mode.as_str().to_string(),
        dictation_trigger_status: trigger_runtime.status,
        dictation_trigger_permission_hint: trigger_runtime.permission_hint,
        focused_field_insert_enabled: focused_field_insert_enabled(&settings),
        focused_field_insert_permission_granted: focused_field_permission.granted,
        focused_field_insert_permission_status: focused_field_permission.status,
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

fn downmix_samples<T, F>(data: &[T], channels: usize, to_f32: F) -> Vec<f32>
where
    T: Copy,
    F: Fn(T) -> f32,
{
    if channels == 0 || data.is_empty() {
        return Vec::new();
    }

    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
    for frame in data.chunks(channels) {
        let sum: f32 = frame.iter().map(|sample| to_f32(*sample)).sum();
        mono.push(sum / frame.len() as f32);
    }

    mono
}

fn store_captured_samples(target: &Arc<Mutex<Vec<f32>>>, samples: &[f32]) {
    if samples.is_empty() {
        return;
    }

    if let Ok(mut guard) = target.lock() {
        guard.extend_from_slice(samples);
    }
}

fn audio_level_from_stats(stats: AudioSignalStats) -> f32 {
    let peak = (stats.peak_abs / 0.18).clamp(0.0, 1.0);
    let rms = (stats.rms / 0.06).clamp(0.0, 1.0);
    ((peak * 0.68) + (rms * 0.32)).clamp(0.0, 1.0)
}

fn waveform_bins_from_samples(samples: &[f32], count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    if samples.is_empty() {
        return vec![0.0; count];
    }

    let chunk_len = (samples.len() / count).max(1);
    let mut bins = Vec::with_capacity(count);
    for index in 0..count {
        let start = index * chunk_len;
        let end = ((index + 1) * chunk_len).min(samples.len());
        let slice = if start < samples.len() {
            &samples[start..end.max(start + 1).min(samples.len())]
        } else {
            &samples[samples.len().saturating_sub(1)..]
        };
        let peak = slice
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        let normalized = (peak / 0.18).sqrt().clamp(0.0, 1.0);
        bins.push(normalized);
    }

    bins
}

impl LiveAudioMeter {
    fn emit_samples(&self, samples: &[f32], sample_rate: u32) {
        if samples.is_empty() || sample_rate == 0 {
            return;
        }

        let now = Instant::now();
        let should_emit = if let Ok(mut guard) = self.last_emitted_at.lock() {
            match *guard {
                Some(last)
                    if now.duration_since(last)
                        < Duration::from_millis(LIVE_AUDIO_EMIT_INTERVAL_MS) =>
                {
                    false
                }
                _ => {
                    *guard = Some(now);
                    true
                }
            }
        } else {
            false
        };

        if !should_emit {
            return;
        }

        let stats = analyze_audio_signal(samples, sample_rate);
        let payload = DictationAudioLevelPayload {
            session_id: self.session_id,
            peak_abs: stats.peak_abs,
            rms: stats.rms,
            level: audio_level_from_stats(stats),
            bars: waveform_bins_from_samples(samples, LIVE_AUDIO_BAR_COUNT),
        };
        self.app.emit(DICTATION_AUDIO_LEVEL_EVENT, payload).ok();
    }
}

fn handle_input_chunk<T, F>(
    data: &[T],
    channels: usize,
    target: &Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    meter: &LiveAudioMeter,
    to_f32: F,
) where
    T: Copy,
    F: Fn(T) -> f32,
{
    let mono = downmix_samples(data, channels, to_f32);
    if mono.is_empty() {
        return;
    }

    store_captured_samples(target, &mono);
    meter.emit_samples(&mono, sample_rate);
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

fn device_name(device: &cpal::Device, fallback: &str) -> String {
    device.name().unwrap_or_else(|_| fallback.to_string())
}

fn list_input_devices() -> Vec<DictationInputDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .map(|device| device_name(&device, "default input"));
    let mut devices = Vec::<DictationInputDevice>::new();
    let mut seen_names = HashSet::<String>::new();

    if let Some(name) = default_name.clone() {
        seen_names.insert(name.clone());
        devices.push(DictationInputDevice {
            is_default: true,
            name,
        });
    }

    if let Ok(inputs) = host.input_devices() {
        for device in inputs {
            let name = device_name(&device, "unknown input");
            if !seen_names.insert(name.clone()) {
                continue;
            }
            devices.push(DictationInputDevice {
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
            });
        }
    }

    devices
}

fn create_input_stream_for_device(
    device: &cpal::Device,
    device_name: &str,
    samples: Arc<Mutex<Vec<f32>>>,
    meter: LiveAudioMeter,
) -> Result<(Stream, u32), String> {
    let supported_config = device
        .default_input_config()
        .or_else(|_| choose_input_config(device))
        .map_err(|e| format!("Failed to resolve input config: {e}"))?;
    let sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels() as usize;
    let config: cpal::StreamConfig = supported_config.clone().into();
    let probe_start_len = samples.lock().map(|guard| guard.len()).unwrap_or(0);
    let err_fn = |err| {
        eprintln!("microphone stream error: {err}");
    };

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => {
            let sink = Arc::clone(&samples);
            let live_meter = meter.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        handle_input_chunk(data, channels, &sink, sample_rate, &live_meter, |v| v);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open f32 input stream: {e}"))?
        }
        SampleFormat::I16 => {
            let sink = Arc::clone(&samples);
            let live_meter = meter.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        handle_input_chunk(data, channels, &sink, sample_rate, &live_meter, |v| {
                            v as f32 / i16::MAX as f32
                        });
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to open i16 input stream: {e}"))?
        }
        SampleFormat::U16 => {
            let sink = Arc::clone(&samples);
            let live_meter = meter.clone();
            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        handle_input_chunk(data, channels, &sink, sample_rate, &live_meter, |v| {
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

    stream
        .play()
        .map_err(|e| format!("Failed to start microphone stream: {e}"))?;

    if let Err(error) =
        wait_for_non_silent_input(&samples, probe_start_len, sample_rate, device_name)
    {
        if let Ok(mut guard) = samples.lock() {
            guard.truncate(probe_start_len);
        }
        drop(stream);
        return Err(error);
    }

    Ok((stream, sample_rate))
}

fn wait_for_non_silent_input(
    samples: &Arc<Mutex<Vec<f32>>>,
    start_len: usize,
    sample_rate: u32,
    device_name: &str,
) -> Result<(), String> {
    if sample_rate == 0 {
        return Ok(());
    }

    let min_samples =
        ((sample_rate as u64 * INPUT_STREAM_PROBE_MIN_DURATION_MS as u64) / 1000).max(32) as usize;
    let deadline = Instant::now() + Duration::from_millis(INPUT_STREAM_PROBE_TIMEOUT_MS);
    let mut saw_any_frames = false;

    loop {
        let observed = if let Ok(guard) = samples.lock() {
            if guard.len() <= start_len {
                None
            } else {
                let captured = &guard[start_len..];
                Some((
                    captured.len(),
                    analyze_audio_signal(captured, sample_rate).peak_abs,
                ))
            }
        } else {
            None
        };

        if let Some((captured_len, peak_abs)) = observed {
            saw_any_frames = true;
            if peak_abs > 0.0 {
                return Ok(());
            }
            if captured_len >= min_samples {
                return Err(format!(
                    "Microphone '{}' opened but only produced silent audio frames. On macOS this usually means the selected input route is stale or muted. In Sound > Input, switch to another microphone and back, or choose System Default and retry.",
                    device_name
                ));
            }
        }

        if Instant::now() >= deadline {
            if !saw_any_frames {
                return Err(format!(
                    "Microphone '{}' did not deliver any audio frames after opening. In macOS Settings > Privacy & Security > Microphone, allow this app and retry.",
                    device_name
                ));
            }
            return Err(format!(
                "Microphone '{}' opened but only produced silent audio frames. On macOS this usually means microphone access is blocked or the input route is stale. In Privacy & Security > Microphone, allow this app, then check Sound > Input and retry.",
                device_name
            ));
        }

        thread::sleep(Duration::from_millis(INPUT_STREAM_PROBE_POLL_INTERVAL_MS));
    }
}

#[cfg(target_os = "macos")]
fn microphone_media_type() -> Result<&'static objc2_av_foundation::AVMediaType, String> {
    unsafe {
        AVMediaTypeAudio.ok_or_else(|| {
            "AVFoundation did not expose AVMediaTypeAudio on this macOS build.".to_string()
        })
    }
}

#[cfg(target_os = "macos")]
fn microphone_permission_denied_error() -> String {
    "Microphone permission is denied for this app. In macOS Settings > Privacy & Security > Microphone, allow dicktaint and relaunch the app.".to_string()
}

#[cfg(target_os = "macos")]
fn microphone_permission_restricted_error() -> String {
    "Microphone access is restricted by macOS for this app. Check Privacy & Security > Microphone or system policy restrictions and retry.".to_string()
}

#[cfg(target_os = "macos")]
fn should_focus_main_window_for_microphone_prompt(status: AVAuthorizationStatus) -> bool {
    status == AVAuthorizationStatus::NotDetermined
}

#[cfg(target_os = "macos")]
fn ensure_microphone_access_authorized(app: &tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let tx_main = tx.clone();
    let app_handle = app.clone();

    app.run_on_main_thread(move || {
        let media_type = match microphone_media_type() {
            Ok(value) => value,
            Err(error) => {
                let _ = tx_main.send(Err(error));
                return;
            }
        };

        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };
        if status == AVAuthorizationStatus::Authorized {
            let _ = tx_main.send(Ok(()));
            return;
        }
        if status == AVAuthorizationStatus::Denied {
            let _ = tx_main.send(Err(microphone_permission_denied_error()));
            return;
        }
        if status == AVAuthorizationStatus::Restricted {
            let _ = tx_main.send(Err(microphone_permission_restricted_error()));
            return;
        }
        if !should_focus_main_window_for_microphone_prompt(status) {
            let _ = tx_main.send(Err(format!(
                "Microphone access returned an unknown AVFoundation authorization state ({}).",
                status.0
            )));
            return;
        }

        show_main_window(&app_handle);

        let tx_request = tx_main.clone();
        let handler = RcBlock::new(move |granted| {
            let _ = tx_request.send(if bool::from(granted) {
                Ok(())
            } else {
                Err(microphone_permission_denied_error())
            });
        });
        unsafe {
            AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &handler);
        }
    })
    .map_err(|e| format!("Failed to request microphone access on the macOS main thread: {e}"))?;

    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(result) => result,
        Err(_) => Err(
            "Timed out waiting for macOS microphone permission. Bring dicktaint to the foreground, approve access in Privacy & Security > Microphone, then retry."
                .to_string(),
        ),
    }
}

#[cfg(not(target_os = "macos"))]
fn ensure_microphone_access_authorized(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

fn create_input_stream(
    samples: Arc<Mutex<Vec<f32>>>,
    meter: LiveAudioMeter,
) -> Result<(Stream, u32, String), String> {
    let host = cpal::default_host();
    let preferred_input_name = meter
        .app
        .state::<LocalModelState>()
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .preferred_input_device
        .clone();
    let mut candidate_devices: Vec<(String, cpal::Device)> = Vec::new();

    let default_name = host
        .default_input_device()
        .map(|device| device_name(&device, "default input"));

    if let Some(preferred_name) = preferred_input_name.as_deref() {
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                let name = device_name(&device, "unknown input");
                if name == preferred_name {
                    candidate_devices.push((name, device));
                }
            }
        }
    }

    if let Some(default_device) = host.default_input_device() {
        let name = device_name(&default_device, "default input");
        candidate_devices.push((name, default_device));
    }

    if let Ok(devices) = host.input_devices() {
        for device in devices {
            candidate_devices.push((device_name(&device, "unknown input"), device));
        }
    }

    if candidate_devices.is_empty() {
        return Err(
            "No microphone input device found. In macOS Settings > Sound > Input, select a microphone and retry."
                .to_string(),
        );
    }

    let mut attempts: Vec<String> = Vec::new();
    for (name, device) in candidate_devices {
        match create_input_stream_for_device(&device, &name, Arc::clone(&samples), meter.clone()) {
            Ok((stream, sample_rate)) => return Ok((stream, sample_rate, name)),
            Err(err) => attempts.push(format!("{name}: {err}")),
        }
    }

    let preferred_detail = preferred_input_name
        .as_deref()
        .map(|name| format!(" Preferred input: {name}."))
        .unwrap_or_default();
    let default_detail = default_name
        .as_deref()
        .map(|name| format!(" Default input: {name}."))
        .unwrap_or_default();

    Err(format!(
        "Could not open microphone input on this machine. Tried: {}. \
In macOS Settings > Privacy & Security > Microphone, allow this app/terminal, then pick an input device in Settings > Sound > Input and retry.{}{}",
        attempts.join(" | "),
        preferred_detail,
        default_detail
    ))
}

fn spawn_recording_thread(
    samples: Arc<Mutex<Vec<f32>>>,
    app: tauri::AppHandle,
    session_id: u64,
) -> Result<(mpsc::Sender<()>, thread::JoinHandle<()>, u32, String), String> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (init_tx, init_rx) = mpsc::channel::<Result<(u32, String), String>>();
    let capture_samples = Arc::clone(&samples);
    let meter = LiveAudioMeter {
        app,
        session_id,
        last_emitted_at: Arc::new(Mutex::new(None)),
    };

    let handle = thread::spawn(move || {
        let stream_result = create_input_stream(capture_samples, meter);
        match stream_result {
            Ok((stream, sample_rate, input_device_name)) => {
                let _ = init_tx.send(Ok((sample_rate, input_device_name)));
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(e) => {
                let _ = init_tx.send(Err(e));
            }
        }
    });

    let (sample_rate, input_device_name) = match init_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(value)) => value,
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

    Ok((stop_tx, handle, sample_rate, input_device_name))
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

fn analyze_audio_signal(samples: &[f32], sample_rate: u32) -> AudioSignalStats {
    if samples.is_empty() || sample_rate == 0 {
        return AudioSignalStats {
            peak_abs: 0.0,
            rms: 0.0,
            duration_secs: 0.0,
        };
    }

    let mut peak_abs = 0.0_f32;
    let mut energy = 0.0_f64;
    for sample in samples {
        let abs = sample.abs();
        if abs > peak_abs {
            peak_abs = abs;
        }
        energy += f64::from(*sample) * f64::from(*sample);
    }

    AudioSignalStats {
        peak_abs,
        rms: (energy / samples.len() as f64).sqrt() as f32,
        duration_secs: samples.len() as f32 / sample_rate as f32,
    }
}

fn audio_signal_is_too_quiet(stats: AudioSignalStats) -> bool {
    stats.peak_abs < MIN_TRANSCRIPTION_AUDIO_PEAK && stats.rms < MIN_TRANSCRIPTION_AUDIO_RMS
}

fn normalize_audio_gain(samples: Vec<f32>, stats: AudioSignalStats) -> Vec<f32> {
    if stats.peak_abs <= 0.0 {
        return samples;
    }

    let gain = (TARGET_TRANSCRIPTION_AUDIO_PEAK / stats.peak_abs).min(MAX_TRANSCRIPTION_AUDIO_GAIN);
    if gain <= 1.0 {
        return samples;
    }

    samples
        .into_iter()
        .map(|sample| (sample * gain).clamp(-1.0, 1.0))
        .collect()
}

fn quiet_audio_error(stats: AudioSignalStats, input_device_name: &str) -> String {
    format!(
        "Captured audio from '{}' was too quiet to transcribe (peak {:.4}, rms {:.4}, {:.1}s). Check macOS Sound > Input, confirm the selected microphone, and retry.",
        input_device_name, stats.peak_abs, stats.rms, stats.duration_secs
    )
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

fn transcribe_samples(
    model_path: PathBuf,
    whisper_cli_path: String,
    samples: Vec<f32>,
    sample_rate: u32,
    input_device_name: String,
) -> Result<String, String> {
    let prepared = if sample_rate == WHISPER_SAMPLE_RATE {
        samples
    } else {
        resample_linear(&samples, sample_rate, WHISPER_SAMPLE_RATE)
    };

    if prepared.is_empty() {
        return Err("No audio captured. Check microphone input and try again.".to_string());
    }

    let signal = analyze_audio_signal(&prepared, WHISPER_SAMPLE_RATE);
    if audio_signal_is_too_quiet(signal) {
        return Err(quiet_audio_error(signal, &input_device_name));
    }
    let prepared = normalize_audio_gain(prepared, signal);

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

    let output = Command::new(&whisper_cli_path)
    .arg("-m")
    .arg(&model_path)
    .arg("-f")
    .arg(&wav_path)
    .arg("-l")
    .arg("en")
    .arg("-otxt")
    .arg("-nt")
    .arg("-of")
    .arg(&out_prefix)
    .output()
    .map_err(|e| {
      format!(
        "Failed to execute whisper cli '{whisper_cli_path}': {e}. Install whisper.cpp (whisper-cli) or set WHISPER_CLI_PATH."
      )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let mut detail = String::new();
        if !stderr.is_empty() {
            detail.push_str(&stderr);
        }
        if detail.is_empty() && !stdout.is_empty() {
            detail.push_str(&stdout);
        }
        if detail.is_empty() {
            detail.push_str("no error output");
        }
        let _ = std::fs::remove_file(&wav_path);
        let _ = std::fs::remove_file(&txt_path);
        return Err(format!("whisper-cli transcription failed: {detail}"));
    }

    let transcript = std::fs::read_to_string(&txt_path).map_err(|e| {
        format!(
            "whisper-cli ran but transcript file is missing at {}: {e}",
            txt_path.display()
        )
    })?;

    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(&txt_path);

    let cleaned = normalize_transcript_text(&transcript);
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
    let mut payload =
        build_onboarding_payload(config.inner(), model_state.inner(), hotkey_state.inner())?;
    match apply_registered_hotkey(
        &app,
        hotkey_state.inner(),
        payload.dictation_trigger.as_deref(),
    ) {
        Ok(runtime) => {
            payload.dictation_trigger_mode = runtime.mode.as_str().to_string();
            payload.dictation_trigger_status = runtime.status;
            payload.dictation_trigger_permission_hint = runtime.permission_hint;
        }
        Err(error) => {
            log::warn!("get_dictation_onboarding: failed to apply global hotkey: {error}");
        }
    }
    Ok(payload)
}

#[tauri::command]
fn get_dictation_trigger(
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?
        .clone();
    let runtime = current_trigger_runtime_details(hotkey_state.inner())?;
    Ok(dictation_trigger_payload(&settings, runtime))
}

#[tauri::command]
fn set_dictation_trigger(
    app: tauri::AppHandle,
    trigger: String,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let normalized = normalize_dictation_trigger(&trigger)?;
    let (previous_trigger, previous_trigger_raw, previous_trigger_enabled) = {
        let settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        (
            resolve_effective_dictation_trigger(&settings),
            settings.dictation_trigger.clone(),
            settings.dictation_trigger_enabled,
        )
    };

    let runtime = apply_registered_hotkey(&app, hotkey_state.inner(), Some(&normalized))?;

    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    settings.dictation_trigger = Some(normalized.clone());
    settings.dictation_trigger_enabled = Some(true);
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.dictation_trigger = previous_trigger_raw;
        settings.dictation_trigger_enabled = previous_trigger_enabled;
        drop(settings);
        if let Err(restore_error) =
            apply_registered_hotkey(&app, hotkey_state.inner(), previous_trigger.as_deref())
        {
            log::warn!("set_dictation_trigger: failed to restore previous hotkey after save error: {restore_error}");
        }
        return Err(error);
    }
    Ok(dictation_trigger_payload(&settings, runtime))
}

#[tauri::command]
fn clear_dictation_trigger(
    app: tauri::AppHandle,
    model_state: State<'_, LocalModelState>,
    hotkey_state: State<'_, GlobalHotkeyState>,
) -> Result<DictationTriggerPayload, String> {
    let (previous_trigger, previous_trigger_raw, previous_trigger_enabled) = {
        let settings = model_state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        (
            resolve_effective_dictation_trigger(&settings),
            settings.dictation_trigger.clone(),
            settings.dictation_trigger_enabled,
        )
    };

    let runtime = apply_registered_hotkey(&app, hotkey_state.inner(), None)?;

    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    settings.dictation_trigger = None;
    settings.dictation_trigger_enabled = Some(false);
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.dictation_trigger = previous_trigger_raw;
        settings.dictation_trigger_enabled = previous_trigger_enabled;
        drop(settings);
        if let Err(restore_error) =
            apply_registered_hotkey(&app, hotkey_state.inner(), previous_trigger.as_deref())
        {
            log::warn!(
                "clear_dictation_trigger: failed to restore previous hotkey after save error: {restore_error}"
            );
        }
        return Err(error);
    }
    Ok(dictation_trigger_payload(&settings, runtime))
}

#[tauri::command]
fn set_focused_field_insert_enabled(
    enabled: bool,
    model_state: State<'_, LocalModelState>,
) -> Result<FocusedFieldInsertPayload, String> {
    let permission = focused_field_insert_permission_status(enabled, enabled);
    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    let previous = settings.focused_field_insert_enabled;
    settings.focused_field_insert_enabled = Some(enabled);
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.focused_field_insert_enabled = previous;
        return Err(error);
    }
    Ok(FocusedFieldInsertPayload {
        enabled: focused_field_insert_enabled(&settings),
        permission_granted: permission.granted,
        permission_status: permission.status,
    })
}

#[tauri::command]
fn set_preferred_input_device(
    device_name: Option<String>,
    model_state: State<'_, LocalModelState>,
) -> Result<Option<String>, String> {
    let normalized = device_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let available_devices = list_input_devices();
    if let Some(name) = normalized.as_deref() {
        if !available_devices.iter().any(|device| device.name == name) {
            return Err(format!(
                "Microphone '{}' is not currently available on this machine.",
                name
            ));
        }
    }

    let settings_path = model_state.settings_path.clone();
    let mut settings = model_state
        .settings
        .lock()
        .map_err(|_| "Failed to lock local model settings".to_string())?;
    let previous = settings.preferred_input_device.clone();
    settings.preferred_input_device = normalized.clone();
    if let Err(error) = save_local_settings(&settings_path, &settings) {
        settings.preferred_input_device = previous;
        return Err(error);
    }

    Ok(settings.preferred_input_device.clone())
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
fn insert_text_into_focused_field_impl(text: &str) -> Result<(), String> {
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
fn insert_text_into_focused_field_impl(_text: &str) -> Result<(), String> {
    Err("Focused field insertion is currently supported on macOS desktop only.".to_string())
}

#[tauri::command]
fn insert_text_into_focused_field(
    state: State<'_, LocalModelState>,
    text: String,
) -> Result<(), String> {
    let focused_field_insert_enabled = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| "Failed to lock local model settings".to_string())?;
        focused_field_insert_enabled(&settings)
    };
    if !focused_field_insert_enabled {
        return Err(
            "Focused-field insertion is disabled in settings. Enable \"Dictate Into Focused Field\" to use this command."
                .to_string(),
        );
    }
    insert_text_into_focused_field_impl(&text)
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

fn start_native_dictation_inner(app: &tauri::AppHandle) -> Result<u64, String> {
    let config = app.state::<AppConfig>();
    let model_state = app.state::<LocalModelState>();
    let dictation = app.state::<DictationState>();

    ensure_microphone_access_authorized(app)?;
    resolve_active_model_path(config.inner(), model_state.inner())?;
    let configured_whisper_cli_path = resolve_whisper_cli_path(
        config.whisper_cli_path_override.as_deref(),
        config.bundled_whisper_cli_path.as_deref(),
    );
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);
    ensure_whisper_cli_available(&whisper_cli_path)?;

    let mut guard = dictation
        .active_recording
        .lock()
        .map_err(|_| "Failed to lock dictation state".to_string())?;
    if guard.is_some() {
        return Err("Dictation already running.".to_string());
    }

    let session_id = dictation.next_session_id.fetch_add(1, Ordering::SeqCst);
    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let (stop_tx, thread_handle, sample_rate, input_device_name) =
        spawn_recording_thread(Arc::clone(&samples), app.clone(), session_id)?;
    *guard = Some(ActiveRecording {
        session_id,
        input_device_name,
        stop_tx,
        thread_handle,
        samples,
        sample_rate,
    });
    drop(guard);

    emit_dictation_state(app, "listening", None, None, Some(session_id));
    Ok(session_id)
}

async fn stop_native_dictation_inner(app: tauri::AppHandle) -> Result<String, String> {
    let recording = {
        let dictation = app.state::<DictationState>();
        let mut guard = dictation
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard
            .take()
            .ok_or_else(|| "Dictation is not running.".to_string())?
    };
    let session_id = recording.session_id;

    let _ = recording.stop_tx.send(());
    if recording.thread_handle.join().is_err() {
        emit_dictation_state(
            &app,
            "error",
            Some("Audio capture thread crashed.".into()),
            None,
            Some(session_id),
        );
        return Err("Audio capture thread crashed.".to_string());
    }

    let captured_samples = recording
        .samples
        .lock()
        .map_err(|_| "Failed to read captured audio".to_string())?
        .clone();
    let model_path = {
        let config = app.state::<AppConfig>();
        let model_state = app.state::<LocalModelState>();
        resolve_active_model_path(config.inner(), model_state.inner())?
    };
    let configured_whisper_cli_path = {
        let config = app.state::<AppConfig>();
        resolve_whisper_cli_path(
            config.whisper_cli_path_override.as_deref(),
            config.bundled_whisper_cli_path.as_deref(),
        )
    };
    let whisper_cli_path = detect_whisper_cli_path(&configured_whisper_cli_path)
        .unwrap_or(configured_whisper_cli_path);

    emit_dictation_state(&app, "processing", None, None, Some(session_id));

    let result = tauri::async_runtime::spawn_blocking(move || {
        transcribe_samples(
            model_path,
            whisper_cli_path,
            captured_samples,
            recording.sample_rate,
            recording.input_device_name,
        )
    })
    .await
    .map_err(|e| {
        emit_dictation_state(&app, "error", Some(e.to_string()), None, Some(session_id));
        format!("Failed to run transcription task: {e}")
    })?;

    match result {
        Ok(transcript) => {
            emit_dictation_state(
                &app,
                "idle",
                None,
                Some(transcript.clone()),
                Some(session_id),
            );
            Ok(transcript)
        }
        Err(e) => {
            emit_dictation_state(&app, "error", Some(e.clone()), None, Some(session_id));
            Err(e)
        }
    }
}

fn cancel_native_dictation_inner(app: &tauri::AppHandle) -> Result<(), String> {
    let recording = {
        let dictation = app.state::<DictationState>();
        let mut guard = dictation
            .active_recording
            .lock()
            .map_err(|_| "Failed to lock dictation state".to_string())?;
        guard.take()
    };
    let session_id = recording.as_ref().map(|value| value.session_id);

    if let Some(recording) = recording {
        let _ = recording.stop_tx.send(());
        let _ = recording.thread_handle.join();
    }

    emit_dictation_state(app, "idle", None, None, session_id);
    Ok(())
}

#[tauri::command]
fn start_native_dictation(app: tauri::AppHandle) -> Result<(), String> {
    start_native_dictation_inner(&app).map(|_| ())
}

#[tauri::command]
async fn stop_native_dictation(app: tauri::AppHandle) -> Result<String, String> {
    stop_native_dictation_inner(app).await
}

#[tauri::command]
fn cancel_native_dictation(app: tauri::AppHandle) -> Result<(), String> {
    cancel_native_dictation_inner(&app)
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_audio_signal, audio_signal_is_too_quiet, default_dictation_trigger,
        focused_field_insert_enabled, normalize_audio_gain, normalize_dictation_trigger,
        onboarding_runtime_details, preferred_whisper_cli_names, quiet_audio_error,
        resample_linear, resolve_effective_dictation_trigger, runtime_details_for_trigger,
        wait_for_non_silent_input, whisper_help_text_looks_valid, HotkeyDeliveryMode,
        LocalSettings,
    };
    use std::sync::{Arc, Mutex};

    #[cfg(target_os = "macos")]
    use super::should_focus_main_window_for_microphone_prompt;
    #[cfg(target_os = "macos")]
    use objc2_av_foundation::AVAuthorizationStatus;

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
    fn analyze_audio_signal_reports_peak_rms_and_duration() {
        let samples = vec![0.0_f32, 0.25, -0.5, 0.5];
        let stats = analyze_audio_signal(&samples, 8_000);
        assert!((stats.peak_abs - 0.5).abs() < 0.0001);
        assert!(stats.rms > 0.0);
        assert!(stats.duration_secs > 0.0);
    }

    #[test]
    fn quiet_audio_detection_flags_near_silent_capture() {
        let samples = vec![0.0002_f32; 16_000];
        let stats = analyze_audio_signal(&samples, 16_000);
        assert!(audio_signal_is_too_quiet(stats));
        assert!(quiet_audio_error(stats, "MacBook Pro Microphone").contains("too quiet"));
    }

    #[test]
    fn normalize_audio_gain_boosts_quiet_but_valid_audio() {
        let samples = vec![0.01_f32, -0.02, 0.03, -0.04];
        let stats = analyze_audio_signal(&samples, 16_000);
        assert!(!audio_signal_is_too_quiet(stats));
        let boosted = normalize_audio_gain(samples.clone(), stats);
        let boosted_peak = boosted
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        let original_peak = samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        assert!(boosted_peak > original_peak);
        assert!(boosted_peak <= 0.85);
    }

    #[test]
    fn silent_stream_probe_rejects_zeroed_frames() {
        let samples = Arc::new(Mutex::new(vec![0.0_f32; 4096]));
        let error = wait_for_non_silent_input(&samples, 0, 16_000, "Austin's AirPods").unwrap_err();
        assert!(error.contains("silent audio frames"));
    }

    #[test]
    fn silent_stream_probe_accepts_nonzero_frames() {
        let samples = Arc::new(Mutex::new(vec![0.0_f32, 0.02, -0.01, 0.0]));
        wait_for_non_silent_input(&samples, 0, 16_000, "MacBook Pro Microphone").unwrap();
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
        assert_eq!(
            normalize_dictation_trigger("globe").unwrap(),
            "Fn".to_string()
        );
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

    #[cfg(target_os = "macos")]
    #[test]
    fn microphone_permission_prompt_only_focuses_for_not_determined_status() {
        assert!(should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::NotDetermined
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Authorized
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Denied
        ));
        assert!(!should_focus_main_window_for_microphone_prompt(
            AVAuthorizationStatus::Restricted
        ));
    }

    #[test]
    fn preferred_whisper_cli_names_include_generic_fallback() {
        let names = preferred_whisper_cli_names();
        assert!(names.iter().any(|name| name == "whisper-cli"));
    }
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init();

    let whisper_model_path_override = std::env::var("WHISPER_MODEL_PATH").ok();
    let whisper_cli_path_override = std::env::var("WHISPER_CLI_PATH").ok();

    let builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    dispatch_backend_hotkey_action(app, BackendHotkeyAction::Toggle);
                }
            })
            .build(),
    );

    let app = builder
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
            let initial_dictation_trigger = resolve_effective_dictation_trigger(&initial_settings);

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

            if should_start_hidden() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            if let Err(error) = create_pill_overlay_windows(app.handle()) {
                log::warn!("Failed to create pill overlay windows: {error}");
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                sync_pill_after_main_window_hide(window.app_handle());
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_dictation_onboarding,
            get_dictation_trigger,
            set_dictation_trigger,
            clear_dictation_trigger,
            set_preferred_input_device,
            set_focused_field_insert_enabled,
            open_whisper_setup_page,
            insert_text_into_focused_field,
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
