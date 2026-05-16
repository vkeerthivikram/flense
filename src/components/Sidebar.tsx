interface SidebarProps {
  activeTab: 'scan' | 'clean' | 'history'
  onTabChange: (tab: 'scan' | 'clean' | 'history') => void
  scanCount?: number
  tools: Array<{ name: string; available: boolean }>
}

const ScanIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="14" height="14">
    <circle cx="7" cy="7" r="4.5" stroke="currentColor" strokeWidth="1.5" />
    <line x1="10.5" y1="10.5" x2="14" y2="14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
)

const ReviewIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="14" height="14">
    <path d="M2 4h12M2 8h8M2 12h5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
)

const HistoryIcon = () => (
  <svg viewBox="0 0 16 16" fill="none" width="14" height="14">
    <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.5" />
    <path d="M8 5v3.5l2 1.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
)

export default function Sidebar({ activeTab, onTabChange, scanCount, tools }: SidebarProps) {
  const navItems = [
    { id: 'scan' as const, label: 'Scan Files', icon: <ScanIcon /> },
    { id: 'clean' as const, label: 'Review & Clean', icon: <ReviewIcon />, count: scanCount },
    { id: 'history' as const, label: 'History', icon: <HistoryIcon /> },
  ]

  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '2px' }}>
          <div
            style={{
              width: 20,
              height: 20,
              background: 'var(--accent-primary)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontWeight: 700,
              fontSize: '12px',
              color: 'var(--surface-base)',
              flexShrink: 0,
            }}
          >
            F
          </div>
          <span>FLENSE</span>
        </div>
        <div style={{ fontSize: '0.6rem', color: 'var(--text-muted-dark)', letterSpacing: '0.06em', paddingLeft: '28px' }}>
          Metadata Cleaner
        </div>
      </div>

      <nav className="sidebar-nav">
        {navItems.map((item) => (
          <button
            key={item.id}
            className={`sidebar-nav-item${activeTab === item.id ? ' active' : ''}`}
            onClick={() => onTabChange(item.id)}
            type="button"
            style={{ borderLeft: activeTab === item.id ? '2px solid var(--accent-primary)' : '2px solid transparent' }}
          >
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
              {item.icon}
              {item.label}
              {item.count !== undefined && item.count > 0 && (
                <span
                  style={{
                    marginLeft: 'auto',
                    background: 'var(--accent-primary)',
                    color: 'var(--surface-base)',
                    fontSize: '0.55rem',
                    fontWeight: 700,
                    padding: '1px 5px',
                    borderRadius: '3px',
                    minWidth: '18px',
                    textAlign: 'center',
                  }}
                >
                  {item.count}
                </span>
              )}
            </span>
          </button>
        ))}
      </nav>

      <div className="sidebar-tools">
        <div
          style={{
            fontSize: '0.6rem',
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
            color: 'var(--text-muted-dark)',
            marginBottom: '4px',
            paddingLeft: '4px',
          }}
        >
          External Tools
        </div>
        {tools.map((tool) => (
          <span key={tool.name} className={`tool-chip ${tool.available ? 'tool-chip-ok' : 'tool-chip-warn'}`}>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <span
                style={{
                  width: 5,
                  height: 5,
                  borderRadius: '50%',
                  background: tool.available ? 'var(--accent-primary)' : 'var(--accent-warning)',
                  display: 'inline-block',
                }}
              />
              {tool.name}
            </span>
          </span>
        ))}
      </div>
    </aside>
  )
}