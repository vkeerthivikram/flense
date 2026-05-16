import React from 'react'

interface TopBarProps {
  title: string
  status?: string
  onCancel?: () => void
  isOperating?: boolean
  actionSlot?: React.ReactNode
}

export default function TopBar({ title, status, onCancel, isOperating, actionSlot }: TopBarProps) {
  return (
    <header className="topbar">
      <h1 className="topbar-title">{title}</h1>
      <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-md)' }}>
        {status && (
          <span className="status-dot" title={status} />
        )}
        {actionSlot}
        {isOperating && onCancel && (
          <button className="cancel-btn" onClick={onCancel} type="button">
            Cancel
          </button>
        )}
      </div>
    </header>
  )
}