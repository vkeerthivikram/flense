// src-tauri/src/errors.rs
// Unified error handling using thiserror for automatic Display/From impls.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to read file: {path} — {source}")]
    FileRead {
        path: String,
        source: std::io::Error,
    },

    #[error("Failed to write file: {path} — {source}")]
    FileWrite {
        path: String,
        source: std::io::Error,
    },

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("File appears corrupted or malformed: {0}")]
    CorruptedFile(String),

    #[error("Failed to parse EXIF data: {0}")]
    ExifParse(String),

    #[error("Failed to parse PDF: {0}")]
    PdfParse(String),

    #[error("Failed to parse ID3 tags: {0}")]
    Id3Parse(String),

    #[error("External tool '{tool}' is not installed or not found in PATH.")]
    ToolNotAvailable { tool: String },

    #[error("External tool '{tool}' failed: {message}")]
    ToolExecution { tool: String, message: String },

    #[error("Backup creation failed: {0}")]
    BackupFailed(String),

    #[error("Restore failed: {0}")]
    RestoreFailed(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Path safety violation: {0}")]
    PathSafety(String),

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("Batch processing error: {completed} completed, {failed} failed")]
    BatchPartial { completed: usize, failed: usize },

    #[error("Atomic write verification failed for: {0}")]
    AtomicWriteFailed(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl AppError {
    /// Convert to a user-friendly error message for the frontend.
    pub fn to_user_message(&self) -> String {
        match self {
            AppError::FileNotFound(path) => {
                format!("File not found. It may have been moved or deleted: {}", truncate_path(path))
            }
            AppError::PermissionDenied(path) => {
                format!(
                    "Permission denied. You may need administrator/root access for: {}",
                    truncate_path(path)
                )
            }
            AppError::UnsupportedFormat(ext) => {
                format!("This file format is not supported for metadata cleaning: {}", ext)
            }
            AppError::CorruptedFile(path) => {
                format!("File appears corrupted and cannot be processed: {}", truncate_path(path))
            }
            AppError::ToolNotAvailable { tool } => {
                format!(
                    "The tool '{}' is not installed. Install it to enable advanced metadata cleaning.",
                    tool
                )
            }
            AppError::ToolExecution { tool, message } => {
                format!("{} failed: {}", tool, message)
            }
            AppError::BackupFailed(msg) => format!("Failed to create backup: {}", msg),
            AppError::RestoreFailed(msg) => format!("Failed to restore file: {}", msg),
            AppError::PathSafety(msg) => format!("Path safety error: {}", msg),
            AppError::Cancelled => "Operation cancelled.".to_string(),
            _ => self.to_string(),
        }
    }
}

fn truncate_path(path: &str) -> String {
    if path.len() > 80 {
        format!("...{}", &path[path.len() - 77..])
    } else {
        path.to_string()
    }
}

// Convert anyhow errors into AppError for ergonomic ? usage.
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Unknown(err.to_string())
    }
}

/// Result alias for convenience.
pub type AppResult<T> = Result<T, AppError>;
