import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type {
  BatchScanResult,
  BatchCleanResult,
  CleanConfig,
  HistoryEntry,
  MetadataItem,
  ProgressEvent,
  ToolStatus,
  AppSettings,
  FileScanResult,
} from '../types'

// ─── Scan ───────────────────────────────────────────────────────────────────

export async function scanFiles(paths: string[]): Promise<BatchScanResult> {
  return invoke<BatchScanResult>('scan_files_cmd', { paths })
}

export async function scanSingleFile(path: string): Promise<FileScanResult> {
  return invoke<FileScanResult>('scan_single_file', { path })
}

// ─── Clean ──────────────────────────────────────────────────────────────────

export async function cleanFiles(
  config: CleanConfig,
  selections: Record<string, MetadataItem[]>
): Promise<BatchCleanResult> {
  return invoke<BatchCleanResult>('clean_files_cmd', { config, selections })
}

// ─── History ────────────────────────────────────────────────────────────────

export async function getHistory(limit: number = 100): Promise<HistoryEntry[]> {
  return invoke<HistoryEntry[]>('get_history_cmd', { limit })
}

export async function restoreFile(filePath: string): Promise<string> {
  return invoke<string>('restore_file_cmd', { filePath })
}

// ─── Tools ──────────────────────────────────────────────────────────────────

export async function detectTools(): Promise<ToolStatus[]> {
  return invoke<ToolStatus[]>('detect_tools')
}

// ─── Settings ───────────────────────────────────────────────────────────────

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('get_settings')
}

export async function updateSettings(settings: AppSettings): Promise<void> {
  return invoke<void>('update_settings', { settings })
}

// ─── Cancellation ───────────────────────────────────────────────────────────

export async function cancelOperation(): Promise<void> {
  return invoke<void>('cancel_operation')
}

export async function resetCancel(): Promise<void> {
  return invoke<void>('reset_cancel')
}

// ─── Events ─────────────────────────────────────────────────────────────────

export async function listenScanProgress(
  callback: (event: ProgressEvent) => void
): Promise<() => void> {
  const unlisten = await listen<ProgressEvent>('scan-progress', (e) => callback(e.payload))
  return unlisten
}

export async function listenCleanProgress(
  callback: (event: ProgressEvent) => void
): Promise<() => void> {
  const unlisten = await listen<ProgressEvent>('clean-progress', (e) => callback(e.payload))
  return unlisten
}
