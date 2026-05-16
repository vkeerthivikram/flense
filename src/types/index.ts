// src/types/index.ts
// TypeScript types matching the Rust JSON response shape.

export type FileType = 'image' | 'pdf' | 'audio' | 'video' | 'document' | 'other';

export type MetadataCategory =
  | 'exif'
  | 'gps'
  | 'iptc'
  | 'xmp'
  | 'id3_tags'
  | 'video_container'
  | 'pdf_info'
  | 'document_properties'
  | 'thumbnail'
  | 'unknown';

export type RemovalCapability = 'removable' | 'partial' | 'read_only' | 'unsupported';

export type SupportLevel = 'full' | 'partial' | 'unsupported';

export type BackupMode = 'adjacent' | 'directory';

export type CleanStatus = 'cleaned' | 'skipped' | 'dry_run' | 'failed';

export type OperationType = 'clean' | 'restore';

export type HistoryStatus = 'success' | 'failed' | 'rolled_back';

export interface MetadataItem {
  key: string;
  value: string;
  category: MetadataCategory;
  capability: RemovalCapability;
  selected: boolean;
  warning?: string;
}

export interface CategorySummary {
  category: MetadataCategory;
  item_count: number;
  removable_count: number;
  partial_count: number;
  read_only_count: number;
  unsupported_count: number;
}

export interface FileScanResult {
  file_path: string;
  file_type: FileType;
  file_size_bytes: number;
  last_modified: string | null;
  support_level: SupportLevel;
  metadata_items: MetadataItem[];
  category_summary: CategorySummary[];
  errors: string[];
  warnings: string[];
}

export interface BatchScanResult {
  files: FileScanResult[];
  total_scanned: number;
  total_with_metadata: number;
  total_errors: number;
  total_warnings: number;
}

export interface CleanConfig {
  dry_run: boolean;
  backup_mode: BackupMode;
  backup_directory?: string;
  file_paths: string[];
  audit_logging: boolean;
}

export interface CleanFileResult {
  file_path: string;
  status: CleanStatus;
  backup_path: string | null;
  metadata_removed_count: number;
  error?: string;
  warning?: string;
}

export interface BatchCleanResult {
  total_files: number;
  files_cleaned: number;
  files_skipped: number;
  files_failed: number;
  dry_run: boolean;
  per_file_results: CleanFileResult[];
  warnings: string[];
}

export interface HistoryEntry {
  id: number;
  operation_id: string;
  file_path: string;
  backup_path: string;
  timestamp: string;
  hash_before: string;
  hash_after: string | null;
  operation_type: OperationType;
  status: HistoryStatus;
  error_message?: string;
  metadata_summary: CategorySummary[];
}

export interface ProgressEvent {
  operation: 'scan' | 'clean';
  current: number;
  total: number;
  current_file: string;
  percentage: number;
}

export interface ToolStatus {
  name: string;
  available: boolean;
  path: string | null;
  version: string | null;
}

export interface AppSettings {
  backup_mode: BackupMode;
  backup_directory: string | null;
  audit_logging: boolean;
  max_concurrency: number;
  use_exiftool: boolean;
  use_ffmpeg: boolean;
}
