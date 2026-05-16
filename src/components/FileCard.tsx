import { FileScanResult, MetadataItem } from '../types/index.ts'
import MetadataGroup from './MetadataGroup.tsx'

interface FileCardProps {
  file: FileScanResult
  isExpanded: boolean
  onToggleExpand: () => void
  onItemToggle: (key: string) => void
  onGroupSelectAll: (category: string, selected: boolean) => void
}

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

const ImageIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
    <rect x="2" y="2" width="12" height="12" rx="1" stroke="currentColor" strokeWidth="1.5" />
    <circle cx="5.5" cy="5.5" r="1.5" fill="currentColor" />
    <path d="M2 11l3.5-3.5 2.5 2.5 2-2 4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
)

const PdfIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
    <rect x="2" y="1" width="10" height="14" rx="1" stroke="currentColor" strokeWidth="1.5" />
    <path d="M8 1v4h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <path d="M5 8h6M5 10.5h4" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" />
  </svg>
)

const AudioIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
    <path d="M2 5h2.5v6H2zM6.5 4h2.5v8h-2.5zM11 3h2.5v10H11z" fill="currentColor" opacity="0.6" />
  </svg>
)

const VideoIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
    <rect x="1" y="3" width="10" height="10" rx="1" stroke="currentColor" strokeWidth="1.5" />
    <path d="M11 6l4-2v8l-4-2v-4z" fill="currentColor" opacity="0.7" />
  </svg>
)

const DocumentIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
    <path d="M3 2h7l3 3v9a1 1 0 01-1 1H3a1 1 0 01-1-1V3a1 1 0 011-1z" stroke="currentColor" strokeWidth="1.5" />
    <path d="M9 2v4h4" stroke="currentColor" strokeWidth="1.5" />
    <path d="M5 8h6M5 10.5h4" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" />
  </svg>
)

const OtherIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
    <path d="M4 2h5l4 4v8a1 1 0 01-1 1H4a1 1 0 01-1-1V3a1 1 0 011-1z" stroke="currentColor" strokeWidth="1.5" />
    <path d="M9 2v4h4" stroke="currentColor" strokeWidth="1.5" />
  </svg>
)

function getFileTypeIcon(type: string) {
  switch (type) {
    case 'image': return <ImageIcon />
    case 'pdf': return <PdfIcon />
    case 'audio': return <AudioIcon />
    case 'video': return <VideoIcon />
    case 'document': return <DocumentIcon />
    default: return <OtherIcon />
  }
}

function getSupportBadgeClass(level: string): string {
  switch (level) {
    case 'full': return 'support-badge support-full'
    case 'partial': return 'support-badge support-partial'
    case 'unsupported': return 'support-badge support-unsupported'
    default: return 'support-badge'
  }
}

function getSupportLevelLabel(level: string): string {
  switch (level) {
    case 'full': return 'Full'
    case 'partial': return 'Partial'
    case 'unsupported': return 'Unsupported'
    default: return level
  }
}

export default function FileCard({
  file,
  isExpanded,
  onToggleExpand,
  onItemToggle,
  onGroupSelectAll,
}: FileCardProps) {
  const gpsItems = file.metadata_items.filter((i) => i.category === 'gps')
  const removableItems = file.metadata_items.filter((i) => i.capability === 'removable')
  const partialItems = file.metadata_items.filter((i) => i.capability === 'partial')
  const readOnlyItems = file.metadata_items.filter((i) => i.capability === 'read_only')

  const groupedItems: Record<string, MetadataItem[]> = {}
  for (const item of file.metadata_items) {
    const cat = item.category
    if (!groupedItems[cat]) {
      groupedItems[cat] = []
    }
    groupedItems[cat].push(item)
  }

  const categories = Object.keys(groupedItems).sort((a, b) => {
    if (a === 'gps') return -1
    if (b === 'gps') return 1
    return a.localeCompare(b)
  })

  const handleHeaderKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      onToggleExpand()
    }
  }

  return (
    <div className="file-card">
      <div
        className="file-header"
        onClick={onToggleExpand}
        aria-expanded={isExpanded}
        role="button"
        tabIndex={0}
        onKeyDown={handleHeaderKeyDown}
      >
        <span className="file-icon">
          {getFileTypeIcon(file.file_type)}
        </span>

        <span className="file-path" title={file.file_path}>
          {formatPath(file.file_path)}
        </span>

        <span className="file-size">
          {formatBytes(file.file_size_bytes)}
        </span>

        <span className={getSupportBadgeClass(file.support_level)}>
          {getSupportLevelLabel(file.support_level)}
        </span>

        <span style={{ color: 'var(--text-muted-dark)', marginLeft: '4px', fontSize: '0.75rem' }}>
          {isExpanded ? '▼' : '▶'}
        </span>
      </div>

      {isExpanded && (
        <div>
          {file.errors.length > 0 && (
            <div
              style={{
                padding: 'var(--spacing-sm) var(--spacing-lg)',
                background: 'rgba(248, 81, 73, 0.08)',
                borderTop: '1px solid var(--border-subtle)',
                fontSize: '0.75rem',
                color: 'var(--accent-danger)',
              }}
            >
              {file.errors.join('; ')}
            </div>
          )}

          {file.warnings.length > 0 && (
            <div
              style={{
                padding: 'var(--spacing-sm) var(--spacing-lg)',
                background: 'rgba(210, 153, 34, 0.08)',
                borderTop: file.errors.length > 0 ? undefined : '1px solid var(--border-subtle)',
                fontSize: '0.75rem',
                color: 'var(--accent-warning)',
              }}
            >
              {file.warnings.join('; ')}
            </div>
          )}

          <div className="file-stats">
            <div className="file-stat">
              <div
                className="file-stat-val"
                style={gpsItems.length > 0 ? { color: 'var(--accent-danger)' } : undefined}
              >
                {gpsItems.length}
              </div>
              <div className="file-stat-lbl">GPS Items</div>
            </div>

            <div className="file-stat">
              <div
                className="file-stat-val"
                style={{ color: 'var(--accent-primary)' }}
              >
                {removableItems.length}
              </div>
              <div className="file-stat-lbl">Removable</div>
            </div>

            <div className="file-stat">
              <div
                className="file-stat-val"
                style={{ color: 'var(--accent-warning)' }}
              >
                {partialItems.length}
              </div>
              <div className="file-stat-lbl">Partial</div>
            </div>

            <div className="file-stat">
              <div
                className="file-stat-val"
                style={{ color: 'var(--text-muted-dark)' }}
              >
                {readOnlyItems.length}
              </div>
              <div className="file-stat-lbl">Read-only</div>
            </div>
          </div>

          <div style={{ padding: 'var(--spacing-md) var(--spacing-lg)' }}>
            {categories.map((category) => (
              <MetadataGroup
                key={category}
                category={category}
                items={groupedItems[category]}
                isExpanded={category === 'gps'}
                onToggle={() => {}}
                onItemToggle={onItemToggle}
                onSelectAll={(selected) => onGroupSelectAll(category, selected)}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}