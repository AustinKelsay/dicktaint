//! Shared application state: constants, configuration types, and dictation settings.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::sync::atomic::AtomicU64;
use std::thread;
use std::time::Instant;
#[cfg(target_os = "macos")]
use tauri::menu::MenuItem;


pub(crate) const WHISPER_SAMPLE_RATE: u32 = 16_000;
pub(crate) const APP_SETTINGS_DIR: &str = ".dicktaint";
pub(crate) const APP_SETTINGS_FILE: &str = "dictation-settings.json";
pub(crate) const APP_MODELS_DIR: &str = "whisper-models";
pub(crate) const DEFAULT_WHISPER_CLI_PATH: &str = "whisper-cli";
#[cfg(target_os = "macos")]
pub(crate) const DEFAULT_DICTATION_TRIGGER: &str = "Fn";
#[cfg(not(target_os = "macos"))]
pub(crate) const DEFAULT_DICTATION_TRIGGER: &str = "CmdOrCtrl+Shift+D";
pub(crate) const MAX_DICTATION_TRIGGER_LENGTH: usize = 64;
pub(crate) const DICTATION_STATE_EVENT: &str = "dictation:state-changed";
pub(crate) const DICTATION_AUDIO_LEVEL_EVENT: &str = "dictation:audio-level";
pub(crate) const PILL_STATUS_EVENT: &str = "dicktaint://pill-status";
#[cfg(target_os = "macos")]
pub(crate) const MAIN_TRAY_ID: &str = "main-tray";
#[cfg(target_os = "macos")]
pub(crate) const TRAY_MENU_STATUS_ID: &str = "tray-status";
#[cfg(target_os = "macos")]
pub(crate) const TRAY_MENU_OPEN_ID: &str = "tray-open";
#[cfg(target_os = "macos")]
pub(crate) const TRAY_MENU_TOGGLE_ID: &str = "tray-toggle";
#[cfg(target_os = "macos")]
pub(crate) const TRAY_MENU_FORCE_STOP_ID: &str = "tray-force-stop";
#[cfg(target_os = "macos")]
pub(crate) const TRAY_MENU_QUIT_ID: &str = "tray-quit";
pub(crate) const WHISPER_CPP_SETUP_URL: &str = "https://github.com/ggml-org/whisper.cpp#quick-start";
pub(crate) const START_HIDDEN_ENV: &str = "DICKTAINT_START_HIDDEN";
pub(crate) const PILL_WINDOW_LABEL_PREFIX: &str = "pill";
pub(crate) const PILL_WINDOW_BASE_WIDTH: f64 = 108.0;
pub(crate) const PILL_WINDOW_MIN_WIDTH: f64 = 92.0;
pub(crate) const PILL_WINDOW_HEIGHT: f64 = 26.0;
pub(crate) const PILL_WINDOW_BOTTOM_MARGIN: i32 = 14;
pub(crate) const MAX_PILL_WINDOWS: usize = 6;
pub(crate) const MIN_TRANSCRIPTION_AUDIO_PEAK: f32 = 0.008;
pub(crate) const MIN_TRANSCRIPTION_AUDIO_RMS: f32 = 0.0008;
pub(crate) const TARGET_TRANSCRIPTION_AUDIO_PEAK: f32 = 0.85;
pub(crate) const MAX_TRANSCRIPTION_AUDIO_GAIN: f32 = 16.0;
pub(crate) const LIVE_AUDIO_BAR_COUNT: usize = 12;
pub(crate) const LIVE_AUDIO_EMIT_INTERVAL_MS: u64 = 45;
pub(crate) const INPUT_STREAM_PROBE_TIMEOUT_MS: u64 = 1_500;
pub(crate) const INPUT_STREAM_PROBE_POLL_INTERVAL_MS: u64 = 40;
#[derive(Clone, Serialize)]
pub(crate) struct DictationStatePayload {
    pub(crate) state: String,
    pub(crate) error: Option<String>,
    pub(crate) transcript: Option<String>,
    pub(crate) session_id: Option<u64>,
}

#[derive(Clone, Serialize)]
pub(crate) struct DictationAudioLevelPayload {
    pub(crate) session_id: u64,
    pub(crate) peak_abs: f32,
    pub(crate) rms: f32,
    pub(crate) level: f32,
    pub(crate) bars: Vec<f32>,
}

#[derive(Clone, Serialize)]
pub(crate) struct PillStatusPayload {
    pub(crate) message: String,
    pub(crate) state: String,
    pub(crate) visible: bool,
}

#[derive(Clone)]
pub(crate) struct AppConfig {
    pub(crate) whisper_model_path_override: Option<String>,
    pub(crate) whisper_cli_path_override: Option<String>,
    pub(crate) bundled_whisper_cli_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BackendDictationStatus {
    #[default]
    Idle,
    Listening,
    Processing,
    Error,
}

impl BackendDictationStatus {
    pub(crate) fn tray_label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Listening => "Listening",
            Self::Processing => "Transcribing",
            Self::Error => "Error",
        }
    }
}

pub(crate) struct DictationState {
    pub(crate) active_recording: Mutex<Option<ActiveRecording>>,
    pub(crate) backend_status: Mutex<BackendDictationStatus>,
    pub(crate) last_error_message: Mutex<Option<String>>,
    pub(crate) next_session_id: AtomicU64,
}

impl Default for DictationState {
    fn default() -> Self {
        Self {
            active_recording: Mutex::new(None),
            backend_status: Mutex::new(BackendDictationStatus::Idle),
            last_error_message: Mutex::new(None),
            next_session_id: AtomicU64::new(1),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct WhisperModelSpec {
    pub(crate) id: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) whisper_ref: &'static str,
    pub(crate) file_name: &'static str,
    pub(crate) approx_size_gb: f32,
    pub(crate) min_ram_gb: u64,
    pub(crate) recommended_ram_gb: u64,
    pub(crate) speed_note: &'static str,
    pub(crate) quality_note: &'static str,
}

pub(crate) const WHISPER_MODEL_CATALOG: [WhisperModelSpec; 12] = [
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
pub(crate) struct LocalSettings {
    pub(crate) selected_model_id: Option<String>,
    pub(crate) selected_model_path: Option<String>,
    pub(crate) preferred_input_device: Option<String>,
    pub(crate) dictation_trigger: Option<String>,
    pub(crate) dictation_trigger_enabled: Option<bool>,
    pub(crate) focused_field_insert_enabled: Option<bool>,
    pub(crate) pill_visibility_mode: Option<String>,
    pub(crate) menu_bar_mode: Option<String>,
    pub(crate) close_action: Option<String>,
}

pub(crate) struct LocalModelState {
    pub(crate) settings_path: PathBuf,
    pub(crate) models_dir: PathBuf,
    pub(crate) settings: Arc<Mutex<LocalSettings>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PillVisibilityMode {
    Off,
    ActiveOnly,
    Always,
}

impl PillVisibilityMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ActiveOnly => "active-only",
            Self::Always => "always",
        }
    }
}

/// Parses a pill visibility mode string. Rejects unsupported values.
pub(crate) fn parse_pill_visibility_mode(value: &str) -> Result<PillVisibilityMode, String> {
    match value.trim() {
        "off" => Ok(PillVisibilityMode::Off),
        "active-only" => Ok(PillVisibilityMode::ActiveOnly),
        "always" => Ok(PillVisibilityMode::Always),
        other => Err(format!(
            "Unsupported pill visibility mode '{}'. Use off, active-only, or always.",
            other
        )),
    }
}

/// Resolves pill visibility from settings; invalid or missing values default to ActiveOnly.
pub(crate) fn resolve_pill_visibility_mode(settings: &LocalSettings) -> PillVisibilityMode {
    match settings
        .pill_visibility_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => match parse_pill_visibility_mode(value) {
            Ok(mode) => mode,
            Err(_) => {
                log::warn!("Ignoring invalid persisted pill visibility mode '{value}'");
                PillVisibilityMode::ActiveOnly
            }
        },
        None => PillVisibilityMode::ActiveOnly,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuBarMode {
    Always,
    BackgroundOnly,
    Off,
}

impl MenuBarMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::BackgroundOnly => "background-only",
            Self::Off => "off",
        }
    }
}

/// Parses a menu bar mode string. Rejects unsupported values.
pub(crate) fn parse_menu_bar_mode(value: &str) -> Result<MenuBarMode, String> {
    match value.trim() {
        "always" => Ok(MenuBarMode::Always),
        "background-only" => Ok(MenuBarMode::BackgroundOnly),
        "off" => Ok(MenuBarMode::Off),
        other => Err(format!(
            "Unsupported menu bar mode '{}'. Use always, background-only, or off.",
            other
        )),
    }
}

/// Resolves menu bar mode from settings; invalid or missing values default to Always.
pub(crate) fn resolve_menu_bar_mode(settings: &LocalSettings) -> MenuBarMode {
    match settings
        .menu_bar_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => match parse_menu_bar_mode(value) {
            Ok(mode) => mode,
            Err(_) => {
                log::warn!("Ignoring invalid persisted menu bar mode '{value}'");
                MenuBarMode::Always
            }
        },
        None => MenuBarMode::Always,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseAction {
    HideToTray,
    Quit,
}

impl CloseAction {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::HideToTray => "hide-to-tray",
            Self::Quit => "quit",
        }
    }
}

/// Parses a close action string. Rejects unsupported values.
pub(crate) fn parse_close_action(value: &str) -> Result<CloseAction, String> {
    match value.trim() {
        "hide-to-tray" => Ok(CloseAction::HideToTray),
        "quit" => Ok(CloseAction::Quit),
        other => Err(format!(
            "Unsupported close action '{}'. Use hide-to-tray or quit.",
            other
        )),
    }
}

/// Resolves close action from settings. Menu bar Off forces Quit; otherwise defaults to HideToTray.
pub(crate) fn resolve_close_action(settings: &LocalSettings, menu_bar_mode: MenuBarMode) -> CloseAction {
    if menu_bar_mode == MenuBarMode::Off {
        return CloseAction::Quit;
    }

    match settings
        .close_action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => match parse_close_action(value) {
            Ok(action) => action,
            Err(_) => {
                log::warn!("Ignoring invalid persisted close action '{value}'");
                CloseAction::HideToTray
            }
        },
        None => CloseAction::HideToTray,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackgroundUiPreferences {
    pub(crate) pill_visibility_mode: PillVisibilityMode,
    pub(crate) menu_bar_mode: MenuBarMode,
    pub(crate) close_action: CloseAction,
}

/// Resolves the full background UI preference set from persisted local settings.
pub(crate) fn resolve_background_ui_preferences(settings: &LocalSettings) -> BackgroundUiPreferences {
    let menu_bar_mode = resolve_menu_bar_mode(settings);
    BackgroundUiPreferences {
        pill_visibility_mode: resolve_pill_visibility_mode(settings),
        close_action: resolve_close_action(settings, menu_bar_mode),
        menu_bar_mode,
    }
}

#[cfg(target_os = "macos")]
pub(crate) type AppMenuItem = MenuItem<tauri::Wry>;
#[cfg(target_os = "macos")]
pub(crate) type AppTrayIcon = tauri::tray::TrayIcon<tauri::Wry>;

#[cfg(target_os = "macos")]
pub(crate) struct TrayRuntimeState {
    pub(crate) tray_icon: AppTrayIcon,
    pub(crate) status_item: AppMenuItem,
    pub(crate) toggle_item: AppMenuItem,
    pub(crate) force_stop_item: AppMenuItem,
}

#[cfg(target_os = "macos")]
#[derive(Default)]
pub(crate) struct TrayState {
    pub(crate) runtime: Mutex<Option<TrayRuntimeState>>,
}

#[cfg(target_os = "macos")]
pub(crate) type CFAllocatorRef = *mut c_void;
#[cfg(target_os = "macos")]
pub(crate) type CFMachPortRef = *mut c_void;
#[cfg(target_os = "macos")]
pub(crate) type CFRunLoopRef = *mut c_void;
#[cfg(target_os = "macos")]
pub(crate) type CFRunLoopSourceRef = *mut c_void;
#[cfg(target_os = "macos")]
pub(crate) type CFStringRef = *const c_void;
#[cfg(target_os = "macos")]
pub(crate) type CGEventRef = *const c_void;
#[cfg(target_os = "macos")]
pub(crate) type CGEventTapProxy = *const c_void;
#[cfg(target_os = "macos")]
pub(crate) type CGEventMask = u64;
#[cfg(target_os = "macos")]
pub(crate) type CGEventFlags = u64;

#[cfg(target_os = "macos")]
pub(crate) const CG_EVENT_TAP_LOCATION_SESSION: u32 = 1;
#[cfg(target_os = "macos")]
pub(crate) const CG_EVENT_TAP_LOCATION_HID: u32 = 0;
#[cfg(target_os = "macos")]
pub(crate) const CG_EVENT_TAP_PLACEMENT_HEAD_INSERT: u32 = 0;
#[cfg(target_os = "macos")]
pub(crate) const CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
#[cfg(target_os = "macos")]
pub(crate) const CG_EVENT_TYPE_FLAGS_CHANGED: u32 = 12;
#[cfg(target_os = "macos")]
pub(crate) const CG_EVENT_TYPE_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
#[cfg(target_os = "macos")]
pub(crate) const CG_EVENT_TYPE_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

#[cfg(target_os = "macos")]
pub(crate) const MACOS_COMMAND_FLAG_MASK: CGEventFlags = 1 << 20;
#[cfg(target_os = "macos")]
pub(crate) const MACOS_FN_FLAG_MASK: CGEventFlags = 1 << 23;
#[cfg(target_os = "macos")]
pub(crate) const MACOS_NON_FN_MODIFIER_MASK: CGEventFlags = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20);
#[cfg(target_os = "macos")]
pub(crate) const KEYCODE_COMMAND: u16 = 0x37;
#[cfg(target_os = "macos")]
pub(crate) const KEYCODE_V: u16 = 0x09;

#[cfg(target_os = "macos")]
pub(crate) type MacFnEventTapCallback =
    unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[derive(Serialize)]
pub(crate) struct DeviceProfile {
    pub(crate) total_memory_gb: u64,
    pub(crate) logical_cpu_cores: usize,
    pub(crate) architecture: String,
    pub(crate) os: String,
}

#[derive(Serialize)]
pub(crate) struct DictationModelOption {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) whisper_ref: String,
    pub(crate) file_name: String,
    pub(crate) path: String,
    pub(crate) installed: bool,
    pub(crate) likely_runnable: bool,
    pub(crate) recommended: bool,
    pub(crate) approx_size_gb: f32,
    pub(crate) min_ram_gb: u64,
    pub(crate) recommended_ram_gb: u64,
    pub(crate) speed_note: String,
    pub(crate) quality_note: String,
}

#[derive(Serialize)]
pub(crate) struct DictationOnboardingPayload {
    pub(crate) onboarding_required: bool,
    pub(crate) selected_model_id: Option<String>,
    pub(crate) selected_model_path: Option<String>,
    pub(crate) selected_model_exists: bool,
    pub(crate) available_input_devices: Vec<DictationInputDevice>,
    pub(crate) preferred_input_device: Option<String>,
    pub(crate) dictation_trigger: Option<String>,
    pub(crate) default_dictation_trigger: String,
    pub(crate) dictation_trigger_mode: String,
    pub(crate) dictation_trigger_status: String,
    pub(crate) dictation_trigger_permission_hint: Option<String>,
    pub(crate) pill_visibility_mode: String,
    pub(crate) menu_bar_mode: String,
    pub(crate) close_action: String,
    pub(crate) focused_field_insert_enabled: bool,
    pub(crate) focused_field_insert_permission_granted: bool,
    pub(crate) focused_field_insert_permission_status: String,
    pub(crate) whisper_cli_available: bool,
    pub(crate) whisper_cli_path: String,
    pub(crate) models_dir: String,
    pub(crate) device: DeviceProfile,
    pub(crate) models: Vec<DictationModelOption>,
}

#[derive(Serialize)]
pub(crate) struct DictationTriggerPayload {
    pub(crate) trigger: Option<String>,
    pub(crate) default_trigger: String,
    pub(crate) trigger_mode: String,
    pub(crate) trigger_status: String,
    pub(crate) trigger_permission_hint: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct BackgroundUiPreferencesPayload {
    pub(crate) pill_visibility_mode: String,
    pub(crate) menu_bar_mode: String,
    pub(crate) close_action: String,
}

#[derive(Serialize)]
pub(crate) struct FocusedFieldInsertPayload {
    pub(crate) enabled: bool,
    pub(crate) permission_granted: bool,
    pub(crate) permission_status: String,
}

#[derive(Clone)]
pub(crate) struct FocusedFieldInsertPermissionStatus {
    pub(crate) granted: bool,
    pub(crate) status: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct DictationInputDevice {
    pub(crate) name: String,
    pub(crate) is_default: bool,
}

#[derive(Serialize)]
pub(crate) struct DictationModelSelection {
    pub(crate) selected_model_id: String,
    pub(crate) selected_model_path: String,
    pub(crate) installed: bool,
}

#[derive(Serialize)]
pub(crate) struct DictationModelDeletion {
    pub(crate) deleted_model_id: String,
    pub(crate) selected_model_id: Option<String>,
    pub(crate) selected_model_path: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AudioSignalStats {
    pub(crate) peak_abs: f32,
    pub(crate) rms: f32,
    pub(crate) duration_secs: f32,
}

#[derive(Clone)]
pub(crate) struct LiveAudioMeter {
    pub(crate) app: tauri::AppHandle,
    pub(crate) session_id: u64,
    pub(crate) last_emitted_at: Arc<Mutex<Option<Instant>>>,
}

pub(crate) struct ActiveRecording {
    pub(crate) session_id: u64,
    pub(crate) input_device_name: String,
    pub(crate) stop_tx: mpsc::Sender<()>,
    pub(crate) thread_handle: thread::JoinHandle<()>,
    pub(crate) samples: Arc<Mutex<Vec<f32>>>,
    pub(crate) sample_rate: u32,
}

#[derive(Clone, Copy)]
pub(crate) enum BackendHotkeyAction {
    Toggle,
    HoldStart,
    HoldStop,
}

#[cfg(test)]
mod tests {
    use super::{
        parse_close_action, parse_menu_bar_mode, parse_pill_visibility_mode,
        resolve_background_ui_preferences, resolve_close_action, resolve_menu_bar_mode,
        resolve_pill_visibility_mode, CloseAction, LocalSettings, MenuBarMode, PillVisibilityMode,
    };

    #[test]
    fn parse_pill_visibility_mode_accepts_valid_values() {
        assert_eq!(
            parse_pill_visibility_mode("off").unwrap(),
            PillVisibilityMode::Off
        );
        assert_eq!(
            parse_pill_visibility_mode("  active-only  ").unwrap(),
            PillVisibilityMode::ActiveOnly
        );
        assert_eq!(
            parse_pill_visibility_mode("always").unwrap(),
            PillVisibilityMode::Always
        );
    }

    #[test]
    fn parse_pill_visibility_mode_rejects_invalid_values() {
        assert!(parse_pill_visibility_mode("nope").is_err());
        assert!(parse_pill_visibility_mode("").is_err());
    }

    #[test]
    fn parse_menu_bar_mode_accepts_valid_values() {
        assert_eq!(parse_menu_bar_mode("always").unwrap(), MenuBarMode::Always);
        assert_eq!(
            parse_menu_bar_mode("background-only").unwrap(),
            MenuBarMode::BackgroundOnly
        );
        assert_eq!(parse_menu_bar_mode("off").unwrap(), MenuBarMode::Off);
    }

    #[test]
    fn parse_menu_bar_mode_rejects_invalid_values() {
        assert!(parse_menu_bar_mode("sometimes").is_err());
        assert!(parse_menu_bar_mode("").is_err());
    }

    #[test]
    fn parse_close_action_accepts_valid_values() {
        assert_eq!(
            parse_close_action("hide-to-tray").unwrap(),
            CloseAction::HideToTray
        );
        assert_eq!(parse_close_action(" quit ").unwrap(), CloseAction::Quit);
    }

    #[test]
    fn parse_close_action_rejects_invalid_values() {
        assert!(parse_close_action("minimize").is_err());
        assert!(parse_close_action("").is_err());
    }

    #[test]
    fn resolve_modes_default_when_missing_or_invalid() {
        let defaults = LocalSettings::default();
        assert_eq!(
            resolve_pill_visibility_mode(&defaults),
            PillVisibilityMode::ActiveOnly
        );
        assert_eq!(resolve_menu_bar_mode(&defaults), MenuBarMode::Always);
        assert_eq!(
            resolve_close_action(&defaults, MenuBarMode::Always),
            CloseAction::HideToTray
        );

        let invalid = LocalSettings {
            pill_visibility_mode: Some("bogus".to_string()),
            menu_bar_mode: Some("bogus".to_string()),
            close_action: Some("bogus".to_string()),
            ..LocalSettings::default()
        };
        assert_eq!(
            resolve_pill_visibility_mode(&invalid),
            PillVisibilityMode::ActiveOnly
        );
        assert_eq!(resolve_menu_bar_mode(&invalid), MenuBarMode::Always);
        assert_eq!(
            resolve_close_action(&invalid, MenuBarMode::Always),
            CloseAction::HideToTray
        );
    }

    #[test]
    fn resolve_background_ui_preferences_defaults_and_forces_quit_when_menu_bar_off() {
        let preferences = resolve_background_ui_preferences(&LocalSettings::default());
        assert_eq!(
            preferences.pill_visibility_mode,
            PillVisibilityMode::ActiveOnly
        );
        assert_eq!(preferences.menu_bar_mode, MenuBarMode::Always);
        assert_eq!(preferences.close_action, CloseAction::HideToTray);

        let preferences = resolve_background_ui_preferences(&LocalSettings {
            menu_bar_mode: Some("off".to_string()),
            close_action: Some("hide-to-tray".to_string()),
            ..LocalSettings::default()
        });
        assert_eq!(preferences.menu_bar_mode, MenuBarMode::Off);
        assert_eq!(preferences.close_action, CloseAction::Quit);
    }
}
