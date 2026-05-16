// src-tauri/src/commands.rs
// Tauri v2 command handlers exposed to the frontend.
// All filesystem operations stay in Rust.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{Emitter, State};

use crate::cleaner::{clean_file, hash_file};
use crate::errors::AppError;
use crate::external_tools::detect_tool;
use crate::history::{get_history as db_get_history, open_db, record_cleaning, record_restore, get_latest_backup};
use crate::metadata_scanner::{collect_files, scan_file};
use crate::types::{
    AppSettings, BatchCleanResult, BatchScanResult, CleanConfig, FileScanResult, HistoryEntry,
    ProgressEvent, ToolStatus,
};

// ──────────────────────────── App State ──────────────────────────────────────

/// Shared application state managed by Tauri.
pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub db_path: PathBuf,
    pub cancelled: Mutex<bool>,
}

// ──────────────────────────── Scan Commands ──────────────────────────────────

#[tauri::command]
pub async fn scan_files_cmd(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<BatchScanResult, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;

    // Collect all files recursively
    let file_paths = collect_files(&paths).map_err(|e| e.to_user_message())?;
    let total = file_paths.len();

    let mut files = Vec::new();
    let mut total_with_metadata = 0;
    let mut total_errors = 0;
    let mut total_warnings = 0;

    for (i, path) in file_paths.iter().enumerate() {
        // Check cancellation
        {
            let cancelled = state.cancelled.lock().map_err(|e| e.to_string())?;
            if *cancelled {
                return Err("Operation cancelled".to_string());
            }
        }

        let result = scan_file(path);

        if !result.metadata_items.is_empty() {
            total_with_metadata += 1;
        }
        if !result.errors.is_empty() {
            total_errors += 1;
        }
        if !result.warnings.is_empty() {
            total_warnings += 1;
        }

        files.push(result);

        // Emit progress event
        let percentage = if total > 0 {
            ((i + 1) as f64 / total as f64) * 100.0
        } else {
            100.0
        };

        let _ = app.emit(
            "scan-progress",
            ProgressEvent {
                operation: "scan".to_string(),
                current: i + 1,
                total,
                current_file: path.to_string_lossy().to_string(),
                percentage,
            },
        );
    }

    Ok(BatchScanResult {
        files,
        total_scanned: total,
        total_with_metadata,
        total_errors,
        total_warnings,
    })
}

#[tauri::command]
pub async fn scan_single_file(path: String) -> Result<FileScanResult, String> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err(format!("File not found: {}", path));
    }
    Ok(scan_file(&path_buf))
}

// ──────────────────────────── Clean Commands ─────────────────────────────────

#[tauri::command]
pub async fn clean_files_cmd(
    config: CleanConfig,
    selections: HashMap<String, Vec<crate::types::MetadataItem>>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<BatchCleanResult, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;

    let file_paths: Vec<PathBuf> = config.file_paths.iter().map(PathBuf::from).collect();
    let total = file_paths.len();
    let max_concurrency = settings.max_concurrency;

    let mut total_cleaned = 0;
    let mut total_skipped = 0;
    let mut total_failed = 0;
    let mut per_file_results = Vec::new();
    let mut warnings = Vec::new();

    // Open database for recording operations
    let db = open_db(&state.db_path).map_err(|e| e.to_user_message())?;

    for (i, path) in file_paths.iter().enumerate() {
        // Check cancellation
        {
            let cancelled = state.cancelled.lock().map_err(|e| e.to_string())?;
            if *cancelled {
                break;
            }
        }

        let items = selections
            .get(&path.to_string_lossy().to_string())
            .cloned()
            .unwrap_or_default();

        let result = clean_file(path, &config, &items).map_err(|e| e.to_user_message())?;

        match &result.status {
            crate::types::CleanStatus::Cleaned => {
                total_cleaned += 1;

                // Record in history
                if !config.dry_run {
                    let hash_before = hash_file(path).unwrap_or_default();
                    let hash_after = result
                        .backup_path
                        .as_ref()
                        .and_then(|p| hash_file(Path::new(p)).ok());

                    let _ = record_cleaning(
                        &db,
                        &uuid::Uuid::new_v4().to_string(),
                        &result.file_path,
                        result.backup_path.as_deref().unwrap_or(""),
                        &hash_before,
                        hash_after.as_deref(),
                        &[],
                        config.audit_logging,
                    );
                }
            }
            crate::types::CleanStatus::Skipped => total_skipped += 1,
            crate::types::CleanStatus::DryRun => total_cleaned += 1,
            crate::types::CleanStatus::Failed => total_failed += 1,
        }

        if let Some(w) = &result.warning {
            warnings.push(w.clone());
        }

        per_file_results.push(result);

        // Emit progress
        let percentage = if total > 0 {
            ((i + 1) as f64 / total as f64) * 100.0
        } else {
            100.0
        };

        let _ = app.emit(
            "clean-progress",
            ProgressEvent {
                operation: "clean".to_string(),
                current: i + 1,
                total,
                current_file: path.to_string_lossy().to_string(),
                percentage,
            },
        );
    }

    Ok(BatchCleanResult {
        total_files: total,
        files_cleaned: total_cleaned,
        files_skipped: total_skipped,
        files_failed: total_failed,
        dry_run: config.dry_run,
        per_file_results,
        warnings,
    })
}

// ──────────────────────────── History / Restore Commands ─────────────────────

#[tauri::command]
pub async fn get_history_cmd(limit: usize, state: State<'_, AppState>) -> Result<Vec<HistoryEntry>, String> {
    let db = open_db(&state.db_path).map_err(|e| e.to_user_message())?;
    db_get_history(&db, limit).map_err(|e| e.to_user_message())
}

#[tauri::command]
pub async fn restore_file_cmd(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db = open_db(&state.db_path).map_err(|e| e.to_user_message())?;

    let backup_path = get_latest_backup(&db, &file_path)
        .map_err(|e| e.to_user_message())?
        .ok_or_else(|| "No backup found for this file".to_string())?;

    let backup = PathBuf::from(&backup_path);
    if !backup.exists() {
        return Err(format!("Backup file not found: {}", backup_path));
    }

    let target = PathBuf::from(&file_path);

    // Warn if target exists and would be overwritten
    if target.exists() {
        // Overwrite is intentional (user requested restore)
    }

    // Copy backup over current file
    std::fs::copy(&backup, &target).map_err(|e| {
        AppError::RestoreFailed(format!("Failed to restore: {}", e)).to_user_message()
    })?;

    // Record restore in history
    record_restore(&db, &file_path, &backup_path, None)
        .map_err(|e| e.to_user_message())?;

    Ok(format!("Restored from backup: {}", backup_path))
}

// ──────────────────────────── Tool Detection Commands ────────────────────────

#[tauri::command]
pub async fn detect_tools() -> Result<Vec<ToolStatus>, String> {
    Ok(vec![detect_tool("exiftool"), detect_tool("ffmpeg")])
}

// ──────────────────────────── Settings Commands ──────────────────────────────

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut current = state.settings.lock().map_err(|e| e.to_string())?;
    *current = settings;
    Ok(())
}

// ──────────────────────────── Cancellation ───────────────────────────────────

#[tauri::command]
pub async fn cancel_operation(state: State<'_, AppState>) -> Result<(), String> {
    let mut cancelled = state.cancelled.lock().map_err(|e| e.to_string())?;
    *cancelled = true;
    Ok(())
}

#[tauri::command]
pub async fn reset_cancel(state: State<'_, AppState>) -> Result<(), String> {
    let mut cancelled = state.cancelled.lock().map_err(|e| e.to_string())?;
    *cancelled = false;
    Ok(())
}
