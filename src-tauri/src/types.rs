// src-tauri/src/types.rs
// Shared data structures serialized to JSON via serde.
// All paths are stored as Strings to avoid cross-platform serialization issues.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// ──────────────────────────── File Type Detection ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    Image,
    Pdf,
    Audio,
    Video,
    Document,
    Other,
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Image => write!(f, "Image"),
            FileType::Pdf => write!(f, "PDF"),
            FileType::Audio => write!(f, "Audio"),
            FileType::Video => write!(f, "Video"),
            FileType::Document => write!(f, "Document"),
            FileType::Other => write!(f, "Other"),
        }
    }
}

// ──────────────────────────── Metadata Categories ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MetadataCategory {
    Exif,
    Gps,
    Iptc,
    Xmp,
    Id3Tags,
    VideoContainer,
    PdfInfo,
    DocumentProperties,
    Thumbnail,
    Unknown,
}

impl std::fmt::Display for MetadataCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetadataCategory::Exif => write!(f, "EXIF"),
            MetadataCategory::Gps => write!(f, "GPS"),
            MetadataCategory::Iptc => write!(f, "IPTC"),
            MetadataCategory::Xmp => write!(f, "XMP"),
            MetadataCategory::Id3Tags => write!(f, "ID3 Tags"),
            MetadataCategory::VideoContainer => write!(f, "Video Metadata"),
            MetadataCategory::PdfInfo => write!(f, "PDF Info"),
            MetadataCategory::DocumentProperties => write!(f, "Document Properties"),
            MetadataCategory::Thumbnail => write!(f, "Thumbnail"),
            MetadataCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

// ──────────────────────────── Removal Capability ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemovalCapability {
    /// Metadata can be fully removed without affecting file contents.
    Removable,
    /// Only partial removal is possible (e.g., some tags are read-only).
    Partial,
    /// Metadata is read-only and cannot be removed.
    ReadOnly,
    /// Removal is not supported for this metadata type or format.
    Unsupported,
}

impl std::fmt::Display for RemovalCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemovalCapability::Removable => write!(f, "Removable"),
            RemovalCapability::Partial => write!(f, "Partial"),
            RemovalCapability::ReadOnly => write!(f, "Read-Only"),
            RemovalCapability::Unsupported => write!(f, "Unsupported"),
        }
    }
}

// ──────────────────────────── Support Level ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    /// Full scanning and cleaning supported.
    Full,
    /// Scanning supported, cleaning has limitations.
    Partial,
    /// Neither scanning nor cleaning supported for this format.
    Unsupported,
}

impl std::fmt::Display for SupportLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SupportLevel::Full => write!(f, "Full"),
            SupportLevel::Partial => write!(f, "Partial"),
            SupportLevel::Unsupported => write!(f, "Unsupported"),
        }
    }
}

// ──────────────────────────── Individual Metadata Item ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataItem {
    pub key: String,
    /// Human-readable value. Sensitive values are masked by default.
    pub value: String,
    pub category: MetadataCategory,
    pub capability: RemovalCapability,
    /// Whether this item is selected for cleaning in the UI.
    #[serde(default)]
    pub selected: bool,
    /// Optional warning if removing this item has risks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

// ──────────────────────────── Per-File Scan Result ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScanResult {
    pub file_path: String,
    pub file_type: FileType,
    pub file_size_bytes: u64,
    pub last_modified: Option<DateTime<Utc>>,
    pub support_level: SupportLevel,
    pub metadata_items: Vec<MetadataItem>,
    /// Category-level summary counts.
    pub category_summary: Vec<CategorySummary>,
    /// Per-file errors (e.g., corrupted header, permission denied).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub errors: Vec<String>,
    /// Warnings about the file or cleaning operation.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: MetadataCategory,
    pub item_count: usize,
    pub removable_count: usize,
    pub partial_count: usize,
    pub read_only_count: usize,
    pub unsupported_count: usize,
}

// ──────────────────────────── Batch Scan Result ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchScanResult {
    pub files: Vec<FileScanResult>,
    pub total_scanned: usize,
    pub total_with_metadata: usize,
    pub total_errors: usize,
    pub total_warnings: usize,
}

// ──────────────────────────── Cleaning Configuration ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanConfig {
    /// Show what would be removed without modifying files.
    #[serde(default)]
    pub dry_run: bool,
    /// Backup location: "adjacent" puts .bak next to original,
    /// "directory" uses a custom backup dir.
    #[serde(default = "default_backup_mode")]
    pub backup_mode: BackupMode,
    /// Custom backup directory (used when backup_mode is "directory").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_directory: Option<String>,
    /// File paths to clean.
    pub file_paths: Vec<String>,
    /// If true, enable verbose audit logging (stores full metadata values).
    #[serde(default)]
    pub audit_logging: bool,
}

fn default_backup_mode() -> BackupMode {
    BackupMode::Adjacent
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupMode {
    Adjacent,
    Directory,
}

// ──────────────────────────── Batch Clean Result ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCleanResult {
    pub total_files: usize,
    pub files_cleaned: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    pub dry_run: bool,
    pub per_file_results: Vec<CleanFileResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanFileResult {
    pub file_path: String,
    pub status: CleanStatus,
    pub backup_path: Option<String>,
    pub metadata_removed_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanStatus {
    Cleaned,
    Skipped,
    DryRun,
    Failed,
}

// ──────────────────────────── History / Restore ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub operation_id: String,
    pub file_path: String,
    pub backup_path: String,
    pub timestamp: DateTime<Utc>,
    pub hash_before: String,
    pub hash_after: Option<String>,
    pub operation_type: OperationType,
    pub status: HistoryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Only populated if audit logging was enabled.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub metadata_summary: Vec<CategorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Clean,
    Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    Success,
    Failed,
    RolledBack,
}

// ──────────────────────────── Progress Events ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub operation: String, // "scan" | "clean"
    pub current: usize,
    pub total: usize,
    pub current_file: String,
    pub percentage: f64,
}

// ──────────────────────────── App Settings ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_backup_mode")]
    pub backup_mode: BackupMode,
    pub backup_directory: Option<String>,
    #[serde(default)]
    pub audit_logging: bool,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_use_exiftool")]
    pub use_exiftool: bool,
    #[serde(default = "default_use_ffmpeg")]
    pub use_ffmpeg: bool,
}

fn default_max_concurrency() -> usize {
    4
}

fn default_use_exiftool() -> bool {
    false
}

fn default_use_ffmpeg() -> bool {
    false
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            backup_mode: BackupMode::Adjacent,
            backup_directory: None,
            audit_logging: false,
            max_concurrency: 4,
            use_exiftool: false,
            use_ffmpeg: false,
        }
    }
}

// ──────────────────────────── External Tool Status ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}
