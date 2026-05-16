// src-tauri/src/external_tools.rs
// Safe detection and invocation of external tools (exiftool, ffmpeg).
// Checks bundled resources first, then system PATH.
// Never passes user input through a shell — always uses structured arguments.

use std::path::Path;
use std::process::Command;

use crate::errors::{AppError, AppResult};
use crate::types::ToolStatus;

// ──────────────────────────── Bundled Tool Resolution ────────────────────────

/// Resolve the path to a bundled tool.
/// Tauri v2 places bundled resources in:
///   - Windows: <exe_dir>/
///   - Linux:   /usr/lib/<app>/ or /opt/<app>/
///   - macOS:   <app>.app/Contents/Resources/
///
/// During development, tools can be placed in src-tauri/bin/.
fn resolve_bundled_tool(name: &str) -> Option<std::path::PathBuf> {
    // Strategy 1: Check relative to the running binary (production bundle)
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            // Direct sibling to the binary (Windows bundled, Linux installed)
            let candidate = parent.join(tool_filename(name));
            if candidate.is_file() {
                return Some(candidate);
            }

            // In a bin/ subdirectory (dev setup)
            let bin_candidate = parent.join("bin").join(tool_filename(name));
            if bin_candidate.is_file() {
                return Some(bin_candidate);
            }

            // macOS: ../Resources/ (from MacOS/ binary)
            if let Some(grandparent) = parent.parent() {
                let resources = grandparent.join("Resources");
                let mac_candidate = resources.join(tool_filename(name));
                if mac_candidate.is_file() {
                    return Some(mac_candidate);
                }
            }

            // Linux: check ../share/<app>/ or ../lib/<app>/
            if let Some(app_name) = current_exe.file_stem() {
                let share = parent.join("share").join(app_name).join("bin");
                let share_candidate = share.join(tool_filename(name));
                if share_candidate.is_file() {
                    return Some(share_candidate);
                }
            }
        }
    }

    // Strategy 2: Check relative to CWD (development)
    let dev_candidate = Path::new("bin").join(tool_filename(name));
    if dev_candidate.is_file() {
        return Some(dev_candidate);
    }

    None
}

/// Platform-specific tool filename.
fn tool_filename(name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

// ──────────────────────────── Tool Detection ─────────────────────────────────

/// Check if a tool is available — bundled first, then system PATH.
pub fn detect_tool(name: &str) -> ToolStatus {
    // Check bundled first
    if let Some(bundled_path) = resolve_bundled_tool(name) {
        let path_str = bundled_path.to_string_lossy().to_string();
        let version = get_tool_version_at(&bundled_path).ok();
        return ToolStatus {
            name: name.to_string(),
            available: true,
            path: Some(path_str),
            version,
        };
    }

    // Fall back to system PATH
    let path = find_executable_in_path(name);
    let available = path.is_some();
    let version = if available {
        get_tool_version(name).ok()
    } else {
        None
    };

    ToolStatus {
        name: name.to_string(),
        available,
        path,
        version,
    }
}

/// Find the full path of an executable in system PATH.
fn find_executable_in_path(name: &str) -> Option<String> {
    let (cmd, args) = if cfg!(windows) {
        ("where", vec![name])
    } else {
        ("which", vec![name])
    };

    let output = Command::new(cmd).args(&args).output().ok()?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        let path = path.trim().to_string();
        if !path.is_empty() {
            Some(path)
        } else {
            None
        }
    } else {
        None
    }
}

/// Get version from a specific binary path.
fn get_tool_version_at(path: &std::path::Path) -> AppResult<String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|_| AppError::ToolNotAvailable {
            tool: path.to_string_lossy().to_string(),
        })?;

    let version = if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };

    Ok(version.lines().next().unwrap_or("").trim().to_string())
}

/// Get version from a tool found in PATH.
fn get_tool_version(name: &str) -> AppResult<String> {
    let output = Command::new(name)
        .arg("--version")
        .output()
        .map_err(|_| AppError::ToolNotAvailable { tool: name.to_string() })?;

    let version = if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };

    Ok(version.lines().next().unwrap_or("").trim().to_string())
}

/// Resolve the executable path for a tool — bundled or system.
fn resolve_tool(name: &str) -> AppResult<std::path::PathBuf> {
    if let Some(bundled) = resolve_bundled_tool(name) {
        return Ok(bundled);
    }
    if let Some(system_path) = find_executable_in_path(name) {
        return Ok(std::path::PathBuf::from(system_path));
    }
    Err(AppError::ToolNotAvailable { tool: name.to_string() })
}

// ──────────────────────────── exiftool ───────────────────────────────────────

/// Use exiftool to scan metadata from a file.
pub fn exiftool_scan(file_path: &Path) -> AppResult<String> {
    let exe = resolve_tool("exiftool")?;

    // exiftool -json -G <file>
    let output = Command::new(&exe)
        .args(["-json", "-G"])
        .arg(file_path)
        .output()
        .map_err(|e| AppError::ToolExecution {
            tool: "exiftool".to_string(),
            message: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::ToolExecution {
            tool: "exiftool".to_string(),
            message: stderr.trim().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Use exiftool to remove all metadata from a file.
/// Writes to output_path for atomic replacement (never overwrites original directly).
pub fn exiftool_clean(file_path: &Path, output_path: &Path) -> AppResult<()> {
    let exe = resolve_tool("exiftool")?;

    // exiftool -all= -o <output> <input>
    let output = Command::new(&exe)
        .args(["-all="])
        .arg("-o")
        .arg(output_path)
        .arg(file_path)
        .output()
        .map_err(|e| AppError::ToolExecution {
            tool: "exiftool".to_string(),
            message: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::ToolExecution {
            tool: "exiftool".to_string(),
            message: stderr.trim().to_string(),
        });
    }

    Ok(())
}

// ──────────────────────────── ffmpeg ─────────────────────────────────────────

/// Use ffmpeg to scan metadata from a media file.
pub fn ffmpeg_scan(file_path: &Path) -> AppResult<String> {
    let exe = resolve_tool("ffmpeg")?;

    // ffmpeg -i <file> -f null -
    let output = Command::new(&exe)
        .arg("-i")
        .arg(file_path)
        .args(["-f", "null", "-"])
        .output()
        .map_err(|e| AppError::ToolExecution {
            tool: "ffmpeg".to_string(),
            message: e.to_string(),
        })?;

    // ffmpeg outputs metadata to stderr even on success.
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

/// Use ffmpeg to strip container-level metadata from a media file.
pub fn ffmpeg_clean(file_path: &Path, output_path: &Path) -> AppResult<()> {
    let exe = resolve_tool("ffmpeg")?;

    // ffmpeg -i <input> -map 0 -c copy -map_metadata -1 -y <output>
    let output = Command::new(&exe)
        .arg("-i")
        .arg(file_path)
        .args(["-map", "0", "-c", "copy", "-map_metadata", "-1", "-y"])
        .arg(output_path)
        .output()
        .map_err(|e| AppError::ToolExecution {
            tool: "ffmpeg".to_string(),
            message: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::ToolExecution {
            tool: "ffmpeg".to_string(),
            message: stderr.trim().to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_nonexistent_tool() {
        let status = detect_tool("nonexistent_tool_xyz123");
        assert!(!status.available);
    }
}
