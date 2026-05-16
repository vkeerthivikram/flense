import React, { useState, useCallback, useEffect, useRef } from 'react'
import {
  scanFiles,
  cleanFiles,
  getHistory,
  restoreFile,
  detectTools,
  cancelOperation,
  resetCancel,
  listenScanProgress,
  listenCleanProgress,
} from './utils/tauri'
import { open } from '@tauri-apps/plugin-dialog'
import type {
  BatchScanResult,
  FileScanResult,
  HistoryEntry,
  MetadataItem,
  ProgressEvent,
  ToolStatus,
  MetadataCategory,
  RemovalCapability,
  SupportLevel,
  FileType,
} from './types'

type Tab = 'scan' | 'clean' | 'history'

// ─── Utility helpers ────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`
}

function formatPath(path: string): string {
  const parts = path.split(/[\\/]/)
  if (parts.length <= 3) return path
  return `…/${parts.slice(-3).join('/')}`
}

function getCapabilityColor(cap: RemovalCapability): string {
  switch (cap) {
    case 'removable': return '#3fb950'
    case 'partial': return '#d29922'
    case 'read_only': return '#6e7681'
    case 'unsupported': return '#f85149'
  }
}

function getSupportLevelBadge(level: SupportLevel): { label: string; color: string } {
  switch (level) {
    case 'full': return { label: 'Full', color: '#3fb950' }
    case 'partial': return { label: 'Partial', color: '#d29922' }
    case 'unsupported': return { label: 'Unsupported', color: '#f85149' }
  }
}

function getFileTypeIcon(type: FileType): string {
  switch (type) {
    case 'image': return '🖼'
    case 'pdf': return '📄'
    case 'audio': return '🎵'
    case 'video': return '🎬'
    case 'document': return '📝'
    case 'other': return '📎'
  }
}

const CATEGORY_LABELS: Record<MetadataCategory, string> = {
  exif: 'EXIF',
  gps: 'GPS',
  iptc: 'IPTC',
  xmp: 'XMP',
  id3_tags: 'ID3 Tags',
  video_container: 'Video',
  pdf_info: 'PDF Info',
  document_properties: 'Document',
  thumbnail: 'Thumbnail',
  unknown: 'Unknown',
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const baseStyles: Record<string, React.CSSProperties> = {
  layout: {
    display: 'flex',
    height: '100vh',
    width: '100vw',
    background: '#0d1117',
    color: '#e6edf3',
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    fontSize: '14px',
  },
  sidebar: {
    width: '220px',
    minWidth: '220px',
    background: '#161b22',
    borderRight: '1px solid #30363d',
    display: 'flex',
    flexDirection: 'column',
    padding: '16px 0',
  },
  sidebarLogo: {
    padding: '0 20px 24px',
    borderBottom: '1px solid #30363d',
    marginBottom: '8px',
  },
  sidebarLogoTitle: {
    fontSize: '16px',
    fontWeight: 700,
    color: '#58a6ff',
    margin: 0,
  },
  sidebarLogoSubtitle: {
    fontSize: '11px',
    color: '#8b949e',
    marginTop: '2px',
  },
  sidebarNav: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
    padding: '8px 12px',
  },
  main: {
    flex: 1,
    display: 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
  },
  topBar: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    padding: '12px 24px',
    borderBottom: '1px solid #30363d',
    background: '#161b22',
    minHeight: '56px',
  },
  topBarTitle: {
    fontSize: '15px',
    fontWeight: 600,
  },
  topBarActions: {
    display: 'flex',
    gap: '8px',
    alignItems: 'center',
  },
  content: {
    flex: 1,
    overflow: 'auto',
    padding: '24px',
  },
  dropzone: {
    border: '2px dashed #30363d',
    borderRadius: '12px',
    padding: '48px 24px',
    textAlign: 'center',
    cursor: 'pointer',
    transition: 'all 200ms ease',
    background: '#161b22',
  },
  dropzoneActive: {
    borderColor: '#58a6ff',
    background: '#388bfd0d',
  },
  dropzoneIcon: {
    fontSize: '48px',
    marginBottom: '16px',
  },
  dropzoneText: {
    fontSize: '15px',
    color: '#e6edf3',
    marginBottom: '8px',
  },
  dropzoneHint: {
    fontSize: '12px',
    color: '#8b949e',
  },
  btnPrimary: {
    background: '#238636',
    color: '#fff',
    padding: '8px 16px',
    borderRadius: '6px',
    fontWeight: 600,
    fontSize: '13px',
  },
  btnSecondary: {
    background: '#21262d',
    color: '#e6edf3',
    padding: '8px 16px',
    borderRadius: '6px',
    fontWeight: 500,
    fontSize: '13px',
    border: '1px solid #30363d',
  },
  btnDanger: {
    background: '#da3633',
    color: '#fff',
    padding: '8px 16px',
    borderRadius: '6px',
    fontWeight: 600,
    fontSize: '13px',
  },
  progressBar: {
    height: '4px',
    background: '#21262d',
    borderRadius: '2px',
    overflow: 'hidden',
    marginTop: '12px',
  },
  fileCard: {
    background: '#161b22',
    border: '1px solid #30363d',
    borderRadius: '8px',
    marginBottom: '12px',
    overflow: 'hidden',
  },
  fileCardHeader: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    padding: '12px 16px',
    cursor: 'pointer',
    userSelect: 'none',
  },
  fileCardPath: {
    fontSize: '13px',
    fontWeight: 500,
    fontFamily: 'monospace',
    color: '#e6edf3',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    flex: 1,
    marginRight: '12px',
  },
  metadataTable: {
    width: '100%',
    borderCollapse: 'collapse' as const,
    fontSize: '12px',
  },
  metadataTh: {
    textAlign: 'left' as const,
    padding: '8px 12px',
    borderBottom: '1px solid #30363d',
    color: '#8b949e',
    fontWeight: 500,
    background: '#0d1117',
  },
  metadataTd: {
    padding: '6px 12px',
    borderBottom: '1px solid #21262d',
    color: '#e6edf3',
  },
  checkbox: {
    width: '16px',
    height: '16px',
    accentColor: '#58a6ff',
    cursor: 'pointer',
  },
  toolbar: {
    display: 'flex',
    gap: '8px',
    alignItems: 'center',
    marginBottom: '16px',
    flexWrap: 'wrap',
  },
  statsGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))',
    gap: '12px',
    marginBottom: '24px',
  },
  statCard: {
    background: '#161b22',
    border: '1px solid #30363d',
    borderRadius: '8px',
    padding: '16px',
  },
  statValue: {
    fontSize: '24px',
    fontWeight: 700,
    color: '#e6edf3',
  },
  statLabel: {
    fontSize: '12px',
    color: '#8b949e',
    marginTop: '4px',
  },
  historyRow: {
    display: 'grid',
    gridTemplateColumns: '40px 1fr 120px 100px 80px 120px',
    gap: '12px',
    alignItems: 'center',
    padding: '10px 16px',
    borderBottom: '1px solid #21262d',
    fontSize: '12px',
  },
  toolStatus: {
    display: 'flex',
    gap: '12px',
    marginBottom: '16px',
  },
}

// Dynamic style helpers
const navItemStyle = (active: boolean): React.CSSProperties => ({
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  padding: '10px 12px',
  borderRadius: '6px',
  background: active ? '#388bfd26' : 'transparent',
  color: active ? '#58a6ff' : '#8b949e',
  cursor: 'pointer',
  fontSize: '13px',
  fontWeight: active ? 600 : 400,
  border: 'none',
  textAlign: 'left',
  width: '100%',
  transition: 'all 150ms ease',
})

const progressFillStyle = (pct: number): React.CSSProperties => ({
  height: '100%',
  width: `${pct}%`,
  background: 'linear-gradient(90deg, #238636, #58a6ff)',
  borderRadius: '2px',
  transition: 'width 200ms ease',
})

const badgeStyle = (color: string): React.CSSProperties => ({
  display: 'inline-flex',
  alignItems: 'center',
  padding: '2px 8px',
  borderRadius: '12px',
  fontSize: '11px',
  fontWeight: 600,
  background: `${color}20`,
  color,
  marginLeft: '8px',
})

const toggleStyle = (active: boolean): React.CSSProperties => ({
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  padding: '6px 12px',
  borderRadius: '6px',
  background: active ? '#388bfd26' : '#21262d',
  color: active ? '#58a6ff' : '#8b949e',
  fontSize: '12px',
  fontWeight: 500,
  cursor: 'pointer',
  border: active ? '1px solid #388bfd' : '1px solid #30363d',
})

const alertStyle = (type: 'warning' | 'error' | 'success' | 'info'): React.CSSProperties => {
  const bgMap = { warning: '#d299221a', error: '#f851491a', success: '#3fb9501a', info: '#388bfd1a' }
  const borderMap = { warning: '#d2992240', error: '#f8514940', success: '#3fb95040', info: '#388bfd40' }
  const colorMap = { warning: '#d29922', error: '#f85149', success: '#3fb950', info: '#58a6ff' }
  return {
    padding: '12px 16px',
    borderRadius: '8px',
    marginBottom: '16px',
    fontSize: '13px',
    background: bgMap[type],
    border: `1px solid ${borderMap[type]}`,
    color: colorMap[type],
  }
}

const toolChipStyle = (available: boolean): React.CSSProperties => ({
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  padding: '4px 10px',
  borderRadius: '12px',
  fontSize: '11px',
  fontWeight: 600,
  background: available ? '#3fb95020' : '#f8514920',
  color: available ? '#3fb950' : '#f85149',
})

// ─── App Component ──────────────────────────────────────────────────────────

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>('scan')
  const [scanResult, setScanResult] = useState<BatchScanResult | null>(null)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [tools, setTools] = useState<ToolStatus[]>([])
  const [scanning, setScanning] = useState(false)
  const [cleaning, setCleaning] = useState(false)
  const [dryRun, setDryRun] = useState(false)
  const [isDragging, setIsDragging] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [successMsg, setSuccessMsg] = useState<string | null>(null)
  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set())
  const [progress, setProgress] = useState<ProgressEvent | null>(null)

  const dropzoneRef = useRef<HTMLDivElement>(null)

  // ─── Event listeners ────────────────────────────────────────────────────

  useEffect(() => {
    let unlistenScan: (() => void) | undefined
    let unlistenClean: (() => void) | undefined

    listenScanProgress((evt) => {
      setProgress(evt)
    }).then((unlisten) => {
      unlistenScan = unlisten
    })

    listenCleanProgress((evt) => {
      setProgress(evt)
    }).then((unlisten) => {
      unlistenClean = unlisten
    })

    return () => {
      unlistenScan?.()
      unlistenClean?.()
    }
  }, [])

  // Load tools status on mount
  useEffect(() => {
    detectTools()
      .then(setTools)
      .catch(() => {})
  }, [])

  // Load history on tab change
  useEffect(() => {
    if (activeTab === 'history') {
      getHistory(200)
        .then(setHistory)
        .catch(() => {})
    }
  }, [activeTab])

  // ─── Drag and drop ──────────────────────────────────────────────────────

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragging(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragging(false)
  }, [])

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragging(false)
    setError(null)
    setSuccessMsg(null)

    const files = Array.from(e.dataTransfer.files)
    if (files.length === 0) return

    const paths = files.map((f) => (f as any).path || f.name)
    await runScan(paths)
  }, [])

  // ─── File selection via dialog ──────────────────────────────────────────

  const handleSelectFiles = async () => {
    setError(null)
    setSuccessMsg(null)
    try {
      const selected = await open({
        multiple: true,
        directory: false,
      })
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected]
        await runScan(paths as string[])
      }
    } catch (err: any) {
      setError(err?.message || 'Failed to open file dialog')
    }
  }

  const handleSelectDirectory = async () => {
    setError(null)
    setSuccessMsg(null)
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      })
      if (selected) {
        await runScan([selected as string])
      }
    } catch (err: any) {
      setError(err?.message || 'Failed to open directory dialog')
    }
  }

  // ─── Scan ───────────────────────────────────────────────────────────────

  const runScan = async (paths: string[]) => {
    setScanning(true)
    setProgress(null)
    await resetCancel()
    try {
      const result = await scanFiles(paths)
      setScanResult(result)
      setActiveTab('clean')
    } catch (err: any) {
      if (err?.includes?.('cancelled')) {
        setError('Scan cancelled')
      } else {
        setError(err?.message || err || 'Scan failed')
      }
    } finally {
      setScanning(false)
      setProgress(null)
    }
  }

  // ─── Clean ──────────────────────────────────────────────────────────────

  const handleClean = async () => {
    if (!scanResult) return
    setCleaning(true)
    setProgress(null)
    await resetCancel()
    setError(null)
    setSuccessMsg(null)

    try {
      // Build selections map from selected items
      const selections: Record<string, MetadataItem[]> = {}
      for (const file of scanResult.files) {
        const selectedItems = file.metadata_items.filter((item) => item.selected)
        if (selectedItems.length > 0) {
          selections[file.file_path] = selectedItems
        }
      }

      const result = await cleanFiles(
        {
          dry_run: dryRun,
          backup_mode: 'adjacent',
          file_paths: scanResult.files.map((f) => f.file_path),
          audit_logging: false,
        },
        selections
      )

      if (result.files_failed === 0) {
        setSuccessMsg(
          dryRun
            ? `Dry run complete. ${result.files_cleaned} files would be cleaned.`
            : `Cleaned ${result.files_cleaned} files. ${result.files_skipped} skipped.`
        )
      } else {
        setError(
          `${result.files_failed} files failed. ${result.files_cleaned} cleaned, ${result.files_skipped} skipped.`
        )
      }
    } catch (err: any) {
      setError(err?.message || err || 'Clean failed')
    } finally {
      setCleaning(false)
      setProgress(null)
    }
  }

  // ─── Restore ────────────────────────────────────────────────────────────

  const handleRestore = async (entry: HistoryEntry) => {
    try {
      await restoreFile(entry.file_path)
      setSuccessMsg(`Restored: ${formatPath(entry.file_path)}`)
      // Reload history
      const updated = await getHistory(200)
      setHistory(updated)
    } catch (err: any) {
      setError(err?.message || 'Restore failed')
    }
  }

  // ─── Selection helpers ──────────────────────────────────────────────────

  const toggleFileExpansion = (path: string) => {
    setExpandedFiles((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const toggleItemSelection = (filePath: string, itemKey: string) => {
    setScanResult((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        files: prev.files.map((f) =>
          f.file_path === filePath
            ? {
                ...f,
                metadata_items: f.metadata_items.map((item) =>
                  item.key === itemKey ? { ...item, selected: !item.selected } : item
                ),
              }
            : f
        ),
      }
    })
  }

  const selectAllInFile = (file: FileScanResult, selected: boolean) => {
    setScanResult((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        files: prev.files.map((f) =>
          f.file_path === file.file_path
            ? {
                ...f,
                metadata_items: f.metadata_items.map((item) =>
                  item.capability === 'removable' ? { ...item, selected } : item
                ),
              }
            : f
        ),
      }
    })
  }

  // ─── Cancel ─────────────────────────────────────────────────────────────

  const handleCancel = async () => {
    await cancelOperation()
  }

  // ─── Render ─────────────────────────────────────────────────────────────

  const totalRemovable =
    scanResult?.files.reduce(
      (sum, f) => sum + f.metadata_items.filter((i) => i.selected).length,
      0
    ) ?? 0

  return (
    <div style={baseStyles.layout}>
      {/* ─── Sidebar ─────────────────────────────────────────────────── */}
      <aside style={baseStyles.sidebar}>
        <div style={baseStyles.sidebarLogo}>
          <h1 style={baseStyles.sidebarLogoTitle}>Metadata Cleaner</h1>
          <p style={baseStyles.sidebarLogoSubtitle}>Scan • Review • Clean</p>
        </div>

        <nav style={baseStyles.sidebarNav}>
          <button
            style={navItemStyle(activeTab === 'scan')}
            onClick={() => setActiveTab('scan')}
          >
            <span>🔍</span> Scan Files
          </button>
          <button
            style={navItemStyle(activeTab === 'clean')}
            onClick={() => setActiveTab('clean')}
            disabled={!scanResult}
          >
            <span>🧹</span> Review & Clean
            {scanResult && (
              <span style={{ marginLeft: 'auto', fontSize: '11px', opacity: 0.7 }}>
                {scanResult.files.length}
              </span>
            )}
          </button>
          <button
            style={navItemStyle(activeTab === 'history')}
            onClick={() => setActiveTab('history')}
          >
            <span>📋</span> History
          </button>
        </nav>

        <div style={{ marginTop: 'auto', padding: '16px 20px', borderTop: '1px solid #30363d' }}>
          <div style={baseStyles.toolStatus}>
            {tools.map((tool) => (
              <div key={tool.name} style={toolChipStyle(tool.available)}>
                <span style={{ fontSize: '10px' }}>{tool.available ? '●' : '○'}</span>
                {tool.name}
              </div>
            ))}
          </div>
        </div>
      </aside>

      {/* ─── Main ────────────────────────────────────────────────────── */}
      <main style={baseStyles.main}>
        {/* Top bar */}
        <header style={baseStyles.topBar}>
          <span style={baseStyles.topBarTitle}>
            {activeTab === 'scan' && 'Scan Files'}
            {activeTab === 'clean' && 'Review & Clean'}
            {activeTab === 'history' && 'Cleaning History'}
          </span>
          <div style={baseStyles.topBarActions}>
            {(scanning || cleaning) && (
              <button style={baseStyles.btnDanger} onClick={handleCancel}>
                Cancel
              </button>
            )}
            {progress && (
              <span style={{ fontSize: '12px', color: '#8b949e' }}>
                {Math.round(progress.percentage)}% — {formatPath(progress.current_file)}
              </span>
            )}
          </div>
        </header>

        {/* Progress bar */}
        {progress && (
          <div style={baseStyles.progressBar}>
            <div style={progressFillStyle(progress.percentage)} />
          </div>
        )}

        {/* Content */}
        <div style={baseStyles.content}>
          {/* Alerts */}
          {error && (
            <div style={alertStyle('error')} role="alert">
              {error}
              <button
                style={{
                  marginLeft: '12px',
                  background: 'none',
                  border: 'none',
                  color: 'inherit',
                  cursor: 'pointer',
                  fontSize: '12px',
                }}
                onClick={() => setError(null)}
              >
                Dismiss
              </button>
            </div>
          )}
          {successMsg && (
            <div style={alertStyle('success')} role="status">
              {successMsg}
              <button
                style={{
                  marginLeft: '12px',
                  background: 'none',
                  border: 'none',
                  color: 'inherit',
                  cursor: 'pointer',
                  fontSize: '12px',
                }}
                onClick={() => setSuccessMsg(null)}
              >
                Dismiss
              </button>
            </div>
          )}

          {/* ─── Scan Tab ────────────────────────────────────────────── */}
          {activeTab === 'scan' && (
            <>
              <div
                ref={dropzoneRef}
                style={{
                  ...baseStyles.dropzone,
                  ...(isDragging ? baseStyles.dropzoneActive : {}),
                }}
                onDragOver={handleDragOver}
                onDragLeave={handleDragLeave}
                onDrop={handleDrop}
                role="region"
                aria-label="Drop zone for files"
              >
                <div style={baseStyles.dropzoneIcon}>📂</div>
                <p style={baseStyles.dropzoneText}>
                  Drop files or folders here to scan for metadata
                </p>
                <p style={baseStyles.dropzoneHint}>
                  Supports images, PDFs, audio, video, and documents
                </p>
              </div>

              <div style={{ display: 'flex', gap: '12px', marginTop: '16px' }}>
                <button style={baseStyles.btnPrimary} onClick={handleSelectFiles}>
                  Select Files
                </button>
                <button style={baseStyles.btnSecondary} onClick={handleSelectDirectory}>
                  Select Folder
                </button>
              </div>

              {scanning && (
                <div style={{ marginTop: '24px', textAlign: 'center' }}>
                  <div style={{ fontSize: '14px', color: '#8b949e' }}>Scanning files…</div>
                  <div
                    style={{
                      marginTop: '12px',
                      width: '200px',
                      height: '4px',
                      background: '#21262d',
                      borderRadius: '2px',
                      overflow: 'hidden',
                      margin: '12px auto 0',
                    }}
                  >
                    <div
                      style={{
                        height: '100%',
                        width: progress ? `${progress.percentage}%` : '0%',
                        background: 'linear-gradient(90deg, #238636, #58a6ff)',
                        borderRadius: '2px',
                        transition: 'width 200ms ease',
                      }}
                    />
                  </div>
                </div>
              )}
            </>
          )}

          {/* ─── Clean Tab ───────────────────────────────────────────── */}
          {activeTab === 'clean' && scanResult && (
            <>
              {/* Stats */}
              <div style={baseStyles.statsGrid}>
                <div style={baseStyles.statCard}>
                  <div style={baseStyles.statValue}>{scanResult.total_scanned}</div>
                  <div style={baseStyles.statLabel}>Files Scanned</div>
                </div>
                <div style={baseStyles.statCard}>
                  <div style={{ ...baseStyles.statValue, color: '#58a6ff' }}>
                    {scanResult.total_with_metadata}
                  </div>
                  <div style={baseStyles.statLabel}>With Metadata</div>
                </div>
                <div style={baseStyles.statCard}>
                  <div style={{ ...baseStyles.statValue, color: '#d29922' }}>
                    {scanResult.total_errors}
                  </div>
                  <div style={baseStyles.statLabel}>Errors</div>
                </div>
                <div style={baseStyles.statCard}>
                  <div style={{ ...baseStyles.statValue, color: '#3fb950' }}>
                    {totalRemovable}
                  </div>
                  <div style={baseStyles.statLabel}>Items Selected</div>
                </div>
              </div>

              {/* Toolbar */}
              <div style={baseStyles.toolbar}>
                <button
                  style={toggleStyle(dryRun)}
                  onClick={() => setDryRun(!dryRun)}
                >
                  <span>{dryRun ? '🛡️' : '🔬'}</span>
                  {dryRun ? 'Dry Run ON' : 'Dry Run OFF'}
                </button>

                <button
                  style={baseStyles.btnSecondary}
                  onClick={() => selectAllInFile(scanResult.files[0], true)}
                >
                  Select All Removable
                </button>

                <button
                  style={baseStyles.btnSecondary}
                  onClick={() =>
                    setScanResult((prev) =>
                      prev
                        ? {
                            ...prev,
                            files: prev.files.map((f) => ({
                              ...f,
                              metadata_items: f.metadata_items.map((i) => ({
                                ...i,
                                selected: false,
                              })),
                            })),
                          }
                        : prev
                    )
                  }
                >
                  Deselect All
                </button>

                <div style={{ flex: 1 }} />

                <button
                  style={baseStyles.btnPrimary}
                  onClick={handleClean}
                  disabled={cleaning || totalRemovable === 0}
                >
                  {cleaning
                    ? 'Cleaning…'
                    : dryRun
                      ? `Dry Run (${totalRemovable} items)`
                      : `Clean ${totalRemovable} Items`}
                </button>
              </div>

              {/* File cards */}
              {scanResult.files.map((file) => {
                const isExpanded = expandedFiles.has(file.file_path)
                const supportBadge = getSupportLevelBadge(file.support_level)

                return (
                  <div key={file.file_path} style={baseStyles.fileCard}>
                    <div
                      style={baseStyles.fileCardHeader}
                      onClick={() => toggleFileExpansion(file.file_path)}
                      role="button"
                      aria-expanded={isExpanded}
                    >
                      <span style={{ marginRight: '8px' }}>
                        {getFileTypeIcon(file.file_type)}
                      </span>
                      <span style={baseStyles.fileCardPath}>{formatPath(file.file_path)}</span>
                      <span style={{ fontSize: '11px', color: '#8b949e', marginRight: '8px' }}>
                        {formatBytes(file.file_size_bytes)}
                      </span>
                      <span style={badgeStyle(supportBadge.color)}>
                        {supportBadge.label}
                      </span>
                      <span style={{ marginLeft: '8px', fontSize: '11px', color: '#8b949e' }}>
                        {isExpanded ? '▼' : '▶'}
                      </span>
                    </div>

                    {/* Errors */}
                    {file.errors.length > 0 && (
                      <div
                        style={{
                          padding: '8px 16px',
                          background: '#f8514910',
                          fontSize: '12px',
                          color: '#f85149',
                        }}
                      >
                        {file.errors.join('; ')}
                      </div>
                    )}

                    {/* Warnings */}
                    {file.warnings.length > 0 && (
                      <div
                        style={{
                          padding: '8px 16px',
                          background: '#d2992210',
                          fontSize: '12px',
                          color: '#d29922',
                        }}
                      >
                        {file.warnings.join('; ')}
                      </div>
                    )}

                    {/* Metadata table */}
                    {isExpanded && file.metadata_items.length > 0 && (
                      <div style={{ overflow: 'auto', maxHeight: '400px' }}>
                        <table style={baseStyles.metadataTable}>
                          <thead>
                            <tr>
                              <th style={{ ...baseStyles.metadataTh, width: '40px' }}>
                                <input
                                  type="checkbox"
                                  style={baseStyles.checkbox}
                                  checked={file.metadata_items.every(
                                    (i) => i.capability !== 'removable' || i.selected
                                  )}
                                  onChange={(e) =>
                                    selectAllInFile(file, e.target.checked)
                                  }
                                  aria-label="Select all in file"
                                />
                              </th>
                              <th style={baseStyles.metadataTh}>Key</th>
                              <th style={baseStyles.metadataTh}>Value</th>
                              <th style={baseStyles.metadataTh}>Category</th>
                              <th style={baseStyles.metadataTh}>Capability</th>
                            </tr>
                          </thead>
                          <tbody>
                            {file.metadata_items.map((item) => (
                              <tr
                                key={item.key}
                                style={{
                                  background: item.selected ? '#388bfd0d' : 'transparent',
                                }}
                              >
                                <td style={baseStyles.metadataTd}>
                                  <input
                                    type="checkbox"
                                    style={{
                                      ...baseStyles.checkbox,
                                      opacity:
                                        item.capability === 'removable' ? 1 : 0.3,
                                    }}
                                    checked={item.selected}
                                    disabled={item.capability !== 'removable'}
                                    onChange={() =>
                                      toggleItemSelection(file.file_path, item.key)
                                    }
                                    aria-label={`Select ${item.key}`}
                                  />
                                </td>
                                <td
                                  style={{
                                    ...baseStyles.metadataTd,
                                    fontFamily: 'monospace',
                                    fontSize: '11px',
                                  }}
                                >
                                  {item.key}
                                </td>
                                <td
                                  style={{
                                    ...baseStyles.metadataTd,
                                    maxWidth: '300px',
                                    overflow: 'hidden',
                                    textOverflow: 'ellipsis',
                                    whiteSpace: 'nowrap',
                                    color: '#8b949e',
                                  }}
                                  title={item.value}
                                >
                                  {item.value}
                                </td>
                                <td style={baseStyles.metadataTd}>
                                  <span style={badgeStyle('#58a6ff')}>
                                    {CATEGORY_LABELS[item.category] || item.category}
                                  </span>
                                </td>
                                <td style={baseStyles.metadataTd}>
                                  <span
                                    style={badgeStyle(
                                      getCapabilityColor(item.capability)
                                    )}
                                  >
                                    {item.capability}
                                  </span>
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}

                    {isExpanded && file.metadata_items.length === 0 && (
                      <div
                        style={{
                          padding: '16px',
                          textAlign: 'center',
                          color: '#8b949e',
                          fontSize: '12px',
                        }}
                      >
                        No metadata detected
                      </div>
                    )}
                  </div>
                )
              })}
            </>
          )}

          {activeTab === 'clean' && !scanResult && (
            <div
              style={{
                textAlign: 'center',
                padding: '64px 24px',
                color: '#8b949e',
              }}
            >
              <div style={{ fontSize: '48px', marginBottom: '16px' }}>🔍</div>
              <p>No files scanned yet.</p>
              <button
                style={{ ...baseStyles.btnPrimary, marginTop: '16px' }}
                onClick={() => setActiveTab('scan')}
              >
                Go to Scan
              </button>
            </div>
          )}

          {/* ─── History Tab ─────────────────────────────────────────── */}
          {activeTab === 'history' && (
            <>
              {history.length === 0 ? (
                <div
                  style={{
                    textAlign: 'center',
                    padding: '64px 24px',
                    color: '#8b949e',
                  }}
                >
                  <div style={{ fontSize: '48px', marginBottom: '16px' }}>📋</div>
                  <p>No cleaning history yet.</p>
                </div>
              ) : (
                <>
                  <div
                    style={{
                      display: 'grid',
                      gridTemplateColumns: '40px 1fr 120px 100px 80px 120px',
                      gap: '12px',
                      padding: '8px 16px',
                      borderBottom: '1px solid #30363d',
                      fontSize: '11px',
                      fontWeight: 600,
                      color: '#8b949e',
                      textTransform: 'uppercase',
                      letterSpacing: '0.05em',
                    }}
                  >
                    <span>ID</span>
                    <span>File</span>
                    <span>Backup</span>
                    <span>Type</span>
                    <span>Status</span>
                    <span>Actions</span>
                  </div>

                  <div style={{ maxHeight: 'calc(100vh - 200px)', overflow: 'auto' }}>
                    {history.map((entry) => (
                      <div key={entry.id} style={baseStyles.historyRow}>
                        <span style={{ color: '#8b949e', fontFamily: 'monospace' }}>
                          #{entry.id}
                        </span>
                        <span
                          style={{
                            fontFamily: 'monospace',
                            fontSize: '11px',
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}
                          title={entry.file_path}
                        >
                          {formatPath(entry.file_path)}
                        </span>
                        <span style={{ fontSize: '11px', color: '#8b949e' }}>
                          {entry.backup_path ? formatPath(entry.backup_path) : '—'}
                        </span>
                        <span style={badgeStyle(entry.operation_type === 'clean' ? '#58a6ff' : '#d29922')}>
                          {entry.operation_type}
                        </span>
                        <span
                          style={badgeStyle(
                            entry.status === 'success'
                              ? '#3fb950'
                              : entry.status === 'failed'
                                ? '#f85149'
                                : '#d29922'
                          )}
                        >
                          {entry.status}
                        </span>
                        <span>
                          {entry.operation_type === 'clean' && entry.backup_path && (
                            <button
                              style={{
                                ...baseStyles.btnSecondary,
                                padding: '4px 8px',
                                fontSize: '11px',
                              }}
                              onClick={() => handleRestore(entry)}
                            >
                              Restore
                            </button>
                          )}
                        </span>
                      </div>
                    ))}
                  </div>
                </>
              )}
            </>
          )}
        </div>
      </main>
    </div>
  )
}
