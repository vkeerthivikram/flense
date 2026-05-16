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
import { clearHistory } from './utils/tauri'
import { open } from '@tauri-apps/plugin-dialog'
import type {
  BatchScanResult,
  FileScanResult,
  HistoryEntry,
  MetadataItem,
  ProgressEvent,
  ToolStatus,
} from './types'
import ToastProvider, { useToast } from './components/ToastProvider'
import Sidebar from './components/Sidebar'
import TopBar from './components/TopBar'
import WorkflowBar from './components/WorkflowBar'
import ProgressBar from './components/ProgressBar'
import CleanView from './views/CleanView'

type Tab = 'scan' | 'clean' | 'history'

function formatPath(path: string): string {
  const parts = path.split(/[\\/]/)
  if (parts.length <= 3) return path
  return `…/${parts.slice(-3).join('/')}`
}

// ─── App Component ──────────────────────────────────────────────────────────

function AppContent() {
  const { addToast } = useToast()

  const [activeTab, setActiveTab] = useState<Tab>('scan')
  const [scanResult, setScanResult] = useState<BatchScanResult | null>(null)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [tools, setTools] = useState<ToolStatus[]>([])
  const [scanning, setScanning] = useState(false)
  const [cleaning, setCleaning] = useState(false)
  const [dryRun, setDryRun] = useState(false)
  const [isDragging, setIsDragging] = useState(false)
  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set())
  const [progress, setProgress] = useState<ProgressEvent | null>(null)
  const [clearingHistory, setClearingHistory] = useState(false)

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

    const files = Array.from(e.dataTransfer.files)
    if (files.length === 0) return

    const paths = files.map((f) => (f as any).path || f.name)
    await runScan(paths)
  }, [])

  // ─── File selection via dialog ──────────────────────────────────────────

  const handleSelectFiles = async () => {
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
      addToast(err?.message || 'Failed to open file dialog', 'error')
    }
  }

  const handleSelectDirectory = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      })
      if (selected) {
        await runScan([selected as string])
      }
    } catch (err: any) {
      addToast(err?.message || 'Failed to open directory dialog', 'error')
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
        addToast('Scan cancelled', 'info')
      } else {
        addToast(err?.message || err || 'Scan failed', 'error')
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

    try {
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
        addToast(
          dryRun
            ? `Dry run complete. ${result.files_cleaned} files would be cleaned.`
            : `Cleaned ${result.files_cleaned} files. ${result.files_skipped} skipped.`,
          'success'
        )
      } else {
        addToast(
          `${result.files_failed} files failed. ${result.files_cleaned} cleaned, ${result.files_skipped} skipped.`,
          'error'
        )
      }
    } catch (err: any) {
      addToast(err?.message || err || 'Clean failed', 'error')
    } finally {
      setCleaning(false)
      setProgress(null)
    }
  }

  // ─── Restore ────────────────────────────────────────────────────────────

  const handleRestore = async (entry: HistoryEntry) => {
    try {
      await restoreFile(entry.file_path)
      addToast(`Restored: ${formatPath(entry.file_path)}`, 'success')
      const updated = await getHistory(200)
      setHistory(updated)
    } catch (err: any) {
      addToast(err?.message || 'Restore failed', 'error')
    }
  }

  // ─── Clear History ───────────────────────────────────────────────────────

  const handleClearHistory = async () => {
    setClearingHistory(true)
    try {
      await clearHistory()
      addToast('History cleared', 'success')
      const updated = await getHistory(200)
      setHistory(updated)
    } catch (err: any) {
      addToast(err?.message || 'Failed to clear history', 'error')
    } finally {
      setClearingHistory(false)
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

  const handleGroupSelectAll = (filePath: string, category: string, selected: boolean) => {
    setScanResult((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        files: prev.files.map((f) =>
          f.file_path === filePath
            ? {
                ...f,
                metadata_items: f.metadata_items.map((item) =>
                  item.category === category && item.capability === 'removable'
                    ? { ...item, selected }
                    : item
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

  const getActiveStep = (): 1 | 2 | 3 => {
    if (activeTab === 'scan') return 1
    if (activeTab === 'clean') return cleaning ? 3 : 2
    return 1
  }

  const getTitle = (): string => {
    switch (activeTab) {
      case 'scan': return 'Scan Files'
      case 'clean': return 'Review & Clean'
      case 'history': return 'Cleaning History'
    }
  }

  const renderScanContent = () => (
    <>
      <div
        ref={dropzoneRef}
        className={`dropzone${isDragging ? ' dropzone-active' : ''}`}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        role="region"
        aria-label="Drop zone for files"
      >
        <div className="drop-icon">📂</div>
        <p className="drop-title">Drop files or folders here to scan for metadata</p>
        <p className="drop-sub">Supports images, PDFs, audio, video, and documents</p>
        <div className="format-pills">
          <span className="format-pill">JPG</span>
          <span className="format-pill">PNG</span>
          <span className="format-pill">PDF</span>
          <span className="format-pill">MP3</span>
          <span className="format-pill">MP4</span>
          <span className="format-pill">DOCX</span>
        </div>
      </div>

      <div className="btn-row">
        <button className="btn-primary" onClick={handleSelectFiles}>
          Select Files
        </button>
        <button className="btn-secondary" onClick={handleSelectDirectory}>
          Select Folder
        </button>
      </div>

      {scanning && (
        <div style={{ marginTop: '24px', textAlign: 'center' }}>
          <div style={{ fontSize: '14px', color: 'var(--text-muted-dark)' }}>Scanning files…</div>
          <div
            style={{
              marginTop: '12px',
              width: '200px',
              height: '4px',
              background: 'var(--surface-overlay)',
              borderRadius: '2px',
              overflow: 'hidden',
              margin: '12px auto 0',
            }}
          >
            <div
              style={{
                height: '100%',
                width: progress ? `${progress.percentage}%` : '0%',
                background: 'var(--accent-primary)',
                borderRadius: '2px',
                transition: 'width 200ms ease',
              }}
            />
          </div>
        </div>
      )}
    </>
  )

  const renderHistoryContent = () => (
    <>
      {history.length === 0 ? (
        <div className="empty-state" style={{ textAlign: 'center', padding: '64px 24px', color: 'var(--text-muted-dark)' }}>
          <div style={{ fontSize: '48px', marginBottom: '16px' }}>📋</div>
          <p>No cleaning history yet.</p>
        </div>
      ) : (
        <>
          <div className="history-grid">
            {history.map((entry) => (
              <div key={entry.id} className="history-card">
                <div className="history-card-header">
                  <span className={`history-op-badge ${entry.operation_type === 'clean' ? 'op-clean' : 'op-dryrun'}`}>
                    {entry.operation_type}
                  </span>
                  <span className={`history-status ${entry.status === 'success' ? 'status-success' : 'status-fail'}`}>
                    {entry.status}
                  </span>
                </div>
                <div className="history-path" title={entry.file_path}>
                  {formatPath(entry.file_path)}
                </div>
                <div className="history-meta">
                  {entry.backup_path ? `Backup: ${formatPath(entry.backup_path)}` : 'No backup'}
                </div>
                <div className="history-footer">
                  <span className="history-time">
                    {new Date(entry.timestamp).toLocaleString()}
                  </span>
                  {entry.operation_type === 'clean' && entry.backup_path && (
                    <button
                      className="restore-btn"
                      onClick={() => handleRestore(entry)}
                    >
                      Restore
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
          <button
            className="clear-history-btn"
            onClick={handleClearHistory}
            disabled={clearingHistory}
          >
            {clearingHistory ? 'Clearing…' : 'Clear History'}
          </button>
        </>
      )}
    </>
  )

  const renderContent = () => {
    switch (activeTab) {
      case 'scan':
        return renderScanContent()
      case 'clean':
        return scanResult ? (
          <CleanView
            scanResult={scanResult}
            dryRun={dryRun}
            cleaning={cleaning}
            totalRemovable={totalRemovable}
            expandedFiles={expandedFiles}
            onToggleDryRun={() => setDryRun(!dryRun)}
            onSelectAll={() => {
              if (scanResult.files[0]) {
                selectAllInFile(scanResult.files[0], true)
              }
            }}
            onDeselectAll={() =>
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
            onClean={handleClean}
            onToggleFileExpand={toggleFileExpansion}
            onToggleItemSelection={toggleItemSelection}
            onGroupSelectAll={handleGroupSelectAll}
          />
        ) : (
          <div className="empty-state" style={{ textAlign: 'center', padding: '64px 24px', color: 'var(--text-muted-dark)' }}>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>🔍</div>
            <p>No files scanned yet.</p>
            <button
              className="btn-primary"
              style={{ marginTop: '16px' }}
              onClick={() => setActiveTab('scan')}
            >
              Go to Scan
            </button>
          </div>
        )
      case 'history':
        return renderHistoryContent()
    }
  }

  return (
    <div className="app-layout">
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        scanCount={scanResult?.files.length}
        tools={tools}
      />

      <div className="main-content">
        <TopBar
          title={getTitle()}
          isOperating={scanning || cleaning}
          onCancel={handleCancel}
          status={progress ? `${Math.round(progress.percentage)}% — ${formatPath(progress.current_file)}` : undefined}
          actionSlot={
            activeTab === 'history' && history.length > 0 ? (
              <button
                className="clear-history-btn"
                style={{ padding: '6px 12px', marginRight: '8px' }}
                onClick={handleClearHistory}
                disabled={clearingHistory}
              >
                {clearingHistory ? 'Clearing…' : 'Clear History'}
              </button>
            ) : undefined
          }
        />

        {activeTab !== 'history' && (
          <WorkflowBar
            activeStep={getActiveStep()}
            mode="workflow"
          />
        )}

        <ProgressBar
          progress={progress?.percentage ?? 0}
          visible={!!progress}
        />

        <div style={{ flex: 1, overflow: 'auto', padding: '24px' }}>
          {renderContent()}
        </div>
      </div>
    </div>
  )
}

export default function App() {
  return (
    <ToastProvider>
      <AppContent />
    </ToastProvider>
  )
}