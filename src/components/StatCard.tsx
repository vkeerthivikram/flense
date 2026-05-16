

interface StatCardProps {
  value: number | string
  label: string
  accentColor?: string
}

export default function StatCard({ value, label, accentColor }: StatCardProps) {
  return (
    <div className="stat-card">
      <div
        className="stat-value"
        style={accentColor ? { color: accentColor } : undefined}
      >
        {value}
      </div>
      <div className="stat-label">{label}</div>
    </div>
  )
}