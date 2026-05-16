

interface ProgressBarProps {
  progress: number
  visible: boolean
}

export default function ProgressBar({ progress, visible }: ProgressBarProps) {
  if (!visible) {
    return (
      <div className="progress-bar" style={{ opacity: 0 }}>
        <div className="progress-fill" style={{ width: '0%' }} />
      </div>
    )
  }

  return (
    <div className="progress-bar">
      <div
        className="progress-fill"
        style={{
          width: `${Math.min(100, Math.max(0, progress))}%`,
        }}
      />
    </div>
  )
}