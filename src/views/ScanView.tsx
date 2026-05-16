import React, { useRef } from 'react'
import type { BatchScanResult, ProgressEvent, HistoryEntry } from '../types'

export interface ScanViewProps {
  scanResult: BatchScanResult | null
  scanning: boolean
  progress: ProgressEvent | null
  history: HistoryEntry[]
  onSelectFiles: () => void
  onSelectFolder: () => void
  isDragging: boolean
  onDragOver: (e: React.DragEvent) => void
  onDragLeave: (e: React.DragEvent) => void
  onDrop: (e: React.DragEvent) => void
}

const SUPPORTED_FORMATS = ['JPEG', 'PNG', 'TIFF', 'PDF', 'MP3', 'FLAC', 'MP4', 'DOCX']

function formatPath(path: string): string {
  const parts = path.split(/[\\/]/)
  if (parts.length <= 3) return path
  return `…/${parts.slice(-3).join('/')}`
}

function getRelativeTime(timestamp: string): string {
  const now = Date.now()
  const then = new Date(timestamp).getTime()
  const diff = Math.floor((now - then) / 1000)
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

export default function ScanView({
  scanning,
  history,
  onSelectFiles,
  onSelectFolder,
  isDragging,
  onDragOver,
  onDragLeave,
  onDrop,
}: ScanViewProps) {
  const dropzoneRef = useRef<HTMLDivElement>(null)

  const recentScans = history.slice(0, 5)

  return (
    <div>
      <style>{`
        .dropzone {
          position: relative;
          border: 1px solid #00d08440;
          border-radius: 6px;
          background: linear-gradient(180deg, #00d08408 0%, #00d08403 100%);
          padding: 48px 24px;
          text-align: center;
          cursor: pointer;
          transition: all 200ms ease;
          overflow: hidden;
        }
        .dropzone::before {
          content: '';
          position: absolute;
          left: 0;
          right: 0;
          top: -100%;
          height: 1px;
          background: linear-gradient(90deg, transparent, #00d08460, transparent);
          animation: scanline 2.5s linear infinite;
        }
        @keyframes scanline {
          0% { top: -100%; }
          100% { top: 200%; }
        }
        .dropzone-active {
          border-color: #00d084;
          background: linear-gradient(180deg, #00d08415 0%, #00d08408 100%);
        }
        .drop-icon {
          font-size: 48px;
          color: #00d084;
          margin-bottom: 12px;
          line-height: 1;
        }
        .drop-title {
          font-size: 16px;
          font-weight: 700;
          color: #e6edf3;
          margin-bottom: 6px;
        }
        .drop-sub {
          font-size: 11px;
          color: #6e7681;
          text-transform: uppercase;
          letter-spacing: 0.1em;
        }
        .format-pills {
          display: flex;
          flex-wrap: wrap;
          gap: 8px;
          margin-top: 16px;
        }
        .format-pill {
          padding: 4px 10px;
          font-size: 11px;
          font-weight: 600;
          color: #6e7681;
          background: #21262d;
          border-radius: 4px;
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }
        .btn-row {
          display: flex;
          gap: 12px;
          margin-top: 20px;
        }
        .btn-primary {
          display: inline-flex;
          align-items: center;
          gap: 6px;
          padding: 10px 20px;
          background: #00d084;
          color: #0d1117;
          border: none;
          border-radius: 6px;
          font-size: 13px;
          font-weight: 700;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          cursor: pointer;
          transition: all 150ms ease;
        }
        .btn-primary:hover {
          background: #00e692;
        }
        .btn-primary:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
        .btn-secondary {
          display: inline-flex;
          align-items: center;
          gap: 6px;
          padding: 10px 20px;
          background: #21262d;
          color: #00d084;
          border: 1px solid #00d084;
          border-radius: 6px;
          font-size: 13px;
          font-weight: 600;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          cursor: pointer;
          transition: all 150ms ease;
        }
        .btn-secondary:hover {
          background: #00d08415;
        }
        .recent-panel {
          margin-top: 32px;
        }
        .recent-header {
          font-size: 11px;
          font-weight: 600;
          color: #6e7681;
          text-transform: uppercase;
          letter-spacing: 0.1em;
          margin-bottom: 12px;
        }
        .recent-row {
          display: flex;
          align-items: center;
          gap: 12px;
          padding: 10px 0;
          border-bottom: 1px solid #21262d;
          font-size: 13px;
        }
        .recent-row:last-child {
          border-bottom: none;
        }
        .recent-type {
          width: 24px;
          font-size: 14px;
          color: #8b949e;
          text-align: center;
        }
        .recent-path {
          flex: 1;
          font-family: monospace;
          font-size: 12px;
          color: #e6edf3;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .recent-time {
          font-size: 11px;
          color: #6e7681;
        }
        .scanning-indicator {
          display: flex;
          align-items: center;
          justify-content: center;
          gap: 8px;
          margin-top: 24px;
          color: #8b949e;
          font-size: 14px;
        }
        .scanning-dots::after {
          content: '';
          animation: dots 1.2s steps(4, end) infinite;
        }
        @keyframes dots {
          0% { content: ''; }
          25% { content: '.'; }
          50% { content: '..'; }
          75% { content: '...'; }
          100% { content: ''; }
        }
      `}</style>

      <div
        ref={dropzoneRef}
        className={`dropzone${isDragging ? ' dropzone-active' : ''}`}
        onDragOver={onDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
        role="region"
        aria-label="Drop zone for files"
      >
        <div className="drop-icon">⊕</div>
        <p className="drop-title">Drop files or folders here</p>
        <p className="drop-sub">METADATA DETECTION ACTIVE</p>
      </div>

      <div className="format-pills">
        {SUPPORTED_FORMATS.map((fmt) => (
          <span key={fmt} className="format-pill">{fmt}</span>
        ))}
      </div>

      <div className="btn-row">
        <button className="btn-primary" onClick={onSelectFiles}>
          ⊙ Scan Files
        </button>
        <button className="btn-secondary" onClick={onSelectFolder}>
          ⊞ Select Folder
        </button>
      </div>

      {scanning && (
        <div className="scanning-indicator">
          <span>Scanning files<span className="scanning-dots"></span></span>
        </div>
      )}

      <div className="recent-panel">
        <p className="recent-header">RECENT SCANS</p>
        {recentScans.length === 0 ? (
          <p style={{ color: '#6e7681', fontSize: '13px' }}>No recent scans yet</p>
        ) : (
          recentScans.map((entry) => (
            <div key={entry.id} className="recent-row">
              <span className="recent-type">▯</span>
              <span className="recent-path" title={entry.file_path}>
                {formatPath(entry.file_path)}
              </span>
              <span className="recent-time">
                {getRelativeTime(entry.timestamp)}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  )
}