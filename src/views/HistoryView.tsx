import type { HistoryEntry } from '../types'
import { HistoryCard } from '../components/HistoryCard'

interface HistoryViewProps {
  history: HistoryEntry[];
  onRestore: (entry: HistoryEntry) => void;
  onClearHistory: () => void;
  clearingHistory: boolean;
}

export function HistoryView({ history, onRestore, onClearHistory, clearingHistory }: HistoryViewProps) {
  return (
    <div className="history-view">
      <div className="history-header">
        <h2>Cleaning History</h2>
        {history.length > 0 && (
          <button
            className="clear-history-btn"
            onClick={onClearHistory}
            disabled={clearingHistory}
          >
            {clearingHistory ? 'Clearing…' : 'Clear History'}
          </button>
        )}
      </div>

      {history.length === 0 ? (
        <div className="history-empty">
          No cleaning history yet.
        </div>
      ) : (
        <div className="history-grid">
          {history.map((entry) => (
            <HistoryCard key={entry.id} entry={entry} onRestore={onRestore} />
          ))}
        </div>
      )}
    </div>
  )
}