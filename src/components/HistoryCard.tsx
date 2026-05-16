import type { HistoryEntry } from '../types'

interface HistoryCardProps {
  entry: HistoryEntry;
  onRestore: (entry: HistoryEntry) => void;
}

function formatRelativeTime(isoString: string): string {
  const date = new Date(isoString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const yesterday = new Date(today.getTime() - 24 * 60 * 60 * 1000)
  const entryDate = new Date(date.getFullYear(), date.getMonth(), date.getDate())

  const time = date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })

  if (entryDate.getTime() === today.getTime()) {
    return `Today ${time}`
  } else if (entryDate.getTime() === yesterday.getTime()) {
    return `Yesterday ${time}`
  } else if (diffDays <= 7) {
    return `${diffDays} days ago`
  } else {
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
  }
}

function getItemsRemovedCount(entry: HistoryEntry): number | null {
  if (entry.metadata_summary && entry.metadata_summary.length > 0) {
    return entry.metadata_summary.reduce((sum, cat) => sum + cat.removable_count, 0)
  }
  return null
}

export function HistoryCard({ entry, onRestore }: HistoryCardProps) {
  const opBadgeClass = entry.operation_type === 'clean' ? 'op-clean' : 'op-restore'
  const opLabel = entry.operation_type.toUpperCase()

  const statusClass = entry.status === 'success' ? 'status-success' : entry.status === 'failed' ? 'status-fail' : 'status-skipped'
  const statusIcon = entry.status === 'success' ? '✓' : entry.status === 'failed' ? '✗' : '≈'
  const statusLabel = entry.status === 'success' ? 'Success' : entry.status === 'failed' ? 'Failed' : 'Rolled back'

  const itemsRemoved = getItemsRemovedCount(entry)
  const hasBackup = entry.backup_path && entry.backup_path.length > 0
  const canRestore = entry.operation_type === 'clean' && hasBackup

  return (
    <div className="history-card">
      <div className="history-card-header">
        <span className={`history-op-badge ${opBadgeClass}`}>{opLabel}</span>
        <span className={`history-status ${statusClass}`}>
          {statusIcon} {statusLabel}
        </span>
      </div>

      <div className="history-path" title={entry.file_path}>
        {entry.file_path.length > 60 ? '...' + entry.file_path.slice(-57) : entry.file_path}
      </div>

      <div className="history-meta">
        {itemsRemoved !== null && (
          <span className="meta-item">{itemsRemoved} items removed</span>
        )}
        <span className="meta-item">{hasBackup ? 'Backup kept' : 'No backup'}</span>
      </div>

      {entry.error_message && (
        <div className="history-error">{entry.error_message}</div>
      )}

      <div className="history-footer">
        <span className="history-time">{formatRelativeTime(entry.timestamp)}</span>
        {canRestore && (
          <button
            className="restore-btn"
            onClick={() => onRestore(entry)}
          >
            Restore
          </button>
        )}
      </div>
    </div>
  )
}