//! whisper-cli discovery, validation, and path resolution.

use crate::state::DEFAULT_WHISPER_CLI_PATH;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tauri::Manager;


pub(crate) fn resolve_whisper_cli_path(override_path: Option<&str>, bundled_path: Option<&str>) -> String {
    let preferred = if let Some(path) = override_path.map(str::trim).filter(|v| !v.is_empty()) {
        path.to_string()
    } else if let Some(path) = bundled_path.map(str::trim).filter(|v| !v.is_empty()) {
        path.to_string()
    } else {
        DEFAULT_WHISPER_CLI_PATH.to_string()
    };

    detect_whisper_cli_path(&preferred).unwrap_or(preferred)
}

pub(crate) fn ensure_whisper_cli_available(whisper_cli_path: &str) -> Result<(), String> {
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

pub(crate) fn can_execute_command(executable: &str) -> bool {
    let path = match validate_whisper_cli_candidate(executable) {
        Ok(path) => path,
        Err(_) => return false,
    };
    run_help_probe(&path)
        .map(|output| help_probe_looks_like_whisper_cli(&output))
        .unwrap_or(false)
}

pub(crate) fn run_help_probe(executable: &Path) -> Result<Output, std::io::Error> {
    Command::new(executable).arg("--help").output()
}

pub(crate) fn help_probe_looks_like_whisper_cli(output: &Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    whisper_help_text_looks_valid(&stdout, &stderr)
}

pub(crate) fn help_probe_summary(output: &Output) -> String {
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

pub(crate) fn whisper_help_text_looks_valid(stdout: &str, stderr: &str) -> bool {
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

pub(crate) fn validate_whisper_cli_candidate(candidate: &str) -> Result<PathBuf, String> {
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

pub(crate) fn resolve_command_path(candidate: &str) -> Option<PathBuf> {
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

pub(crate) fn is_explicit_path(value: &str) -> bool {
    Path::new(value).is_absolute() || value.contains('/') || value.contains('\\')
}

pub(crate) fn preferred_arch_variants() -> Vec<&'static str> {
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

pub(crate) fn preferred_whisper_cli_names() -> Vec<String> {
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

pub(crate) fn find_whisper_cli_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in preferred_whisper_cli_names() {
        let preferred = dir.join(name);
        if preferred.is_file() {
            return Some(preferred);
        }
    }
    None
}

pub(crate) fn resolve_bundled_whisper_cli_path(app: &tauri::AppHandle) -> Option<String> {
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

pub(crate) fn local_dev_sidecar_candidates() -> Vec<String> {
    let binaries_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    preferred_whisper_cli_names()
        .into_iter()
        .map(|name| binaries_dir.join(name).to_string_lossy().to_string())
        .collect()
}

pub(crate) fn candidate_whisper_cli_paths(configured_path: &str) -> Vec<String> {
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

pub(crate) fn detect_whisper_cli_path(configured_path: &str) -> Option<String> {
    candidate_whisper_cli_paths(configured_path)
        .into_iter()
        .find(|candidate| can_execute_command(candidate))
}
