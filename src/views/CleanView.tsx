import { useMemo } from 'react'
import { BatchScanResult, FileScanResult } from '../types/index.ts'
import StatCard from '../components/StatCard.tsx'
import FileCard from '../components/FileCard.tsx'

interface CleanViewProps {
  scanResult: BatchScanResult | null
  dryRun: boolean
  cleaning: boolean
  totalRemovable: number
  expandedFiles: Set<string>
  onToggleDryRun: () => void
  onSelectAll: () => void
  onDeselectAll: () => void
  onClean: () => void
  onToggleFileExpand: (path: string) => void
  onToggleItemSelection: (filePath: string, key: string) => void
  onGroupSelectAll: (filePath: string, category: string, selected: boolean) => void
}

function countGpsSensitiveFiles(files: FileScanResult[]): number {
  return files.filter((f) => f.metadata_items.some((i) => i.category === 'gps')).length
}

export default function CleanView({
  scanResult,
  dryRun,
  cleaning,
  totalRemovable,
  expandedFiles,
  onToggleDryRun,
  onSelectAll,
  onDeselectAll,
  onClean,
  onToggleFileExpand,
  onToggleItemSelection,
  onGroupSelectAll,
}: CleanViewProps) {
  const files = scanResult?.files ?? []

  const gpsSensitiveCount = useMemo(
    () => countGpsSensitiveFiles(files),
    [files]
  )

  if (!scanResult || files.length === 0) {
    return (
      <div className="clean-view">
        <div className="empty-state">
          <p>No files scanned yet.</p>
          <p style={{ fontSize: '0.875rem', color: 'var(--text-muted-dark)', marginTop: 'var(--spacing-md)' }}>
            Go to Scan tab to select and scan files.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="clean-view">
      <div className="stats-grid">
        <StatCard value={scanResult.total_scanned} label="Files Scanned" />
        <StatCard
          value={scanResult.total_with_metadata}
          label="With Metadata"
          accentColor="var(--accent-primary)"
        />
        <StatCard
          value={gpsSensitiveCount}
          label="GPS Sensitive"
          accentColor="var(--accent-danger)"
        />
        <StatCard
          value={totalRemovable}
          label="Items Selected"
          accentColor="var(--accent-primary)"
        />
      </div>

      <div className="toolbar">
        <button
          className={`toggle-btn ${dryRun ? 'toggle-btn-on' : ''}`}
          onClick={onToggleDryRun}
        >
          ⛨ Dry Run {dryRun ? 'ON' : 'OFF'}
        </button>

        <button className="btn btn-secondary" onClick={onSelectAll}>
          SELECT ALL
        </button>

        <button className="btn btn-secondary" onClick={onDeselectAll}>
          DESELECT
        </button>

        <div className="toolbar-spacer" />

        <button
          className="clean-btn"
          onClick={onClean}
          disabled={totalRemovable === 0 || cleaning}
        >
          {dryRun ? `Dry Run (${totalRemovable} items)` : `Clean ${totalRemovable} Items`}
        </button>
      </div>

      <div className="file-list">
        {files.map((file) => (
          <FileCard
            key={file.file_path}
            file={file}
            isExpanded={expandedFiles.has(file.file_path)}
            onToggleExpand={() => onToggleFileExpand(file.file_path)}
            onItemToggle={(key) => onToggleItemSelection(file.file_path, key)}
            onGroupSelectAll={(category, selected) =>
              onGroupSelectAll(file.file_path, category, selected)
            }
          />
        ))}
      </div>
    </div>
  )
}