import { MetadataItem } from '../types/index.ts';

interface MetadataGroupProps {
  category: string;
  items: MetadataItem[];
  isExpanded: boolean;
  onToggle: () => void;
  onItemToggle: (key: string) => void;
  onSelectAll: (selected: boolean) => void;
}

function MetadataGroup({
  category,
  items,
  isExpanded,
  onToggle,
  onItemToggle,
  onSelectAll,
}: MetadataGroupProps) {
  const isGps = category.toLowerCase() === 'gps';
  const displayName = isGps ? 'GPS LOCATION' : category.toUpperCase();

  const allReadOnly = items.every((item) => item.capability === 'read_only');
  const removableItems = items.filter((item) => item.capability === 'removable');
  const allRemovableSelected =
    removableItems.length > 0 &&
    removableItems.every((item) => item.selected);

  const handleSelectAll = (e: React.ChangeEvent<HTMLInputElement>) => {
    e.stopPropagation();
    onSelectAll(e.target.checked);
  };

  const handleHeaderKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onToggle();
    }
  };

  const handleRowClick = (key: string) => {
    const item = items.find((i) => i.key === key);
    if (item && item.capability === 'removable') {
      onItemToggle(key);
    }
  };

  return (
    <div className="cat-group">
      <div
        className={`cat-header ${isGps ? 'cat-header-danger' : 'cat-header-normal'}`}
        onClick={onToggle}
        aria-expanded={isExpanded}
        role="button"
        tabIndex={0}
        onKeyDown={handleHeaderKeyDown}
        style={isGps ? { background: 'rgba(248, 81, 73, 0.06)' } : undefined}
      >
        <input
          type="checkbox"
          className="cat-checkbox"
          checked={allRemovableSelected}
          onChange={handleSelectAll}
          disabled={allReadOnly || removableItems.length === 0}
          aria-label={
            allRemovableSelected
              ? `Deselect all ${displayName} items`
              : `Select all ${displayName} items`
          }
          onClick={(e) => e.stopPropagation()}
        />

        <span className={`cat-name ${isGps ? 'cat-name-danger' : 'cat-name-normal'}`}>
          {displayName}
        </span>

        <span
          className={`cat-count ${
            allReadOnly
              ? 'cat-count-muted'
              : isGps
                ? 'cat-count-danger'
                : 'cat-count-normal'
          }`}
        >
          {allReadOnly
            ? `${items.length} read-only`
            : isGps
              ? `${items.length} sensitive`
              : items.length}
        </span>

        {isGps && <span className="sensitive-badge">SENSITIVE</span>}

        <svg
          className={`cat-chevron ${isExpanded ? 'cat-chevron-open' : ''}`}
          viewBox="0 0 16 16"
          fill="currentColor"
          aria-hidden="true"
        >
          <path d="M6 4l4 4-4 4" stroke="currentColor" strokeWidth="1.5" fill="none" />
        </svg>
      </div>

      {isExpanded && (
        <div className="meta-rows">
          {items.map((item) => {
            const isRemovable = item.capability === 'removable';
            return (
              <div
                key={item.key}
                className="meta-row"
                onClick={() => handleRowClick(item.key)}
                style={
                  item.selected
                    ? { background: 'rgba(0, 208, 132, 0.08)' }
                    : undefined
                }
              >
                <input
                  type="checkbox"
                  className="cat-checkbox"
                  checked={item.selected}
                  disabled={!isRemovable}
                  onChange={() => handleRowClick(item.key)}
                  aria-label={`Toggle ${item.key}`}
                  onClick={(e) => e.stopPropagation()}
                  style={!isRemovable ? { opacity: 0.5, pointerEvents: 'none' } : undefined}
                />

                <span className="meta-key">{item.key}</span>

                <span className="meta-val" title={item.value}>
                  {item.value}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default MetadataGroup;