'use client';

import { cn } from '@/lib/utils';
import type { EventNameRow } from '@/lib/api';

interface EventsTableProps {
  rows: EventNameRow[];
  total: number;
  hasMore: boolean;
  loading: boolean;
  selectedEvent: string | null;
  onSelectEvent: (name: string | null) => void;
}

function DeltaBadge({ current, prev }: { current: number; prev?: number }) {
  if (prev === undefined || prev === null) return null;
  if (prev === 0) {
    return <span className="text-xs text-ink-3 font-mono">new</span>;
  }
  const pct = Math.round(((current - prev) / prev) * 100);
  const up = pct >= 0;
  return (
    <span
      className={cn(
        'text-xs font-mono tabular-nums px-1 rounded-sm',
        up ? 'text-spark bg-spark/10' : 'text-red-400 bg-red-400/10'
      )}
    >
      {up ? '+' : ''}
      {pct}%
    </span>
  );
}

function SkeletonRow() {
  return (
    <div className="px-4 py-3 flex items-center gap-2 animate-pulse">
      <div className="flex-1 h-4 bg-surface-2 rounded" />
      <div className="w-16 h-4 bg-surface-2 rounded" />
      <div className="w-16 h-4 bg-surface-2 rounded" />
      <div className="w-12 h-4 bg-surface-2 rounded" />
    </div>
  );
}

function EventsEmptyState() {
  return (
    <div className="px-6 py-12 text-center">
      <p className="text-sm font-medium text-ink mb-1">No custom events yet</p>
      <p className="text-sm text-ink-3 mb-4">
        Track events from your app with one line of code:
      </p>
      <pre className="text-left text-xs bg-surface-2 border border-line rounded-lg p-4 font-mono text-ink-2 overflow-x-auto whitespace-pre">
        {`// @sparklytics/next SDK or vanilla s.js:
window.sparklytics?.track('purchase', {
  plan: 'pro',
  price: 29
})`}
      </pre>
    </div>
  );
}

export function EventsTable({
  rows,
  total,
  hasMore,
  loading,
  selectedEvent,
  onSelectEvent,
}: EventsTableProps) {
  const maxCount = rows[0]?.count ?? 1;

  return (
    <div className="border border-line rounded-lg bg-surface-1">
      {/* Header */}
      <div className="px-4 py-3 border-b border-line flex items-center justify-between">
        <h3 className="text-sm font-medium text-ink">Custom Events</h3>
        {!loading && rows.length > 0 && (
          <span className="text-xs text-ink-3 font-mono tabular-nums">
            {hasMore
              ? `Top ${rows.length} of ${total}`
              : `${total} event${total !== 1 ? 's' : ''}`}
          </span>
        )}
      </div>

      {/* Column headers */}
      {!loading && rows.length > 0 && (
        <div className="px-4 py-2 flex items-center gap-2 text-xs font-medium text-ink-3 uppercase tracking-wider border-b border-line">
          <span className="flex-1">Event</span>
          <span className="w-20 text-right">Visitors</span>
          <span className="w-20 text-right">Count</span>
          <span className="w-16 text-right">vs prev</span>
        </div>
      )}

      {/* Rows */}
      <div className="divide-y divide-line">
        {loading ? (
          Array.from({ length: 5 }).map((_, i) => <SkeletonRow key={i} />)
        ) : rows.length === 0 ? (
          <EventsEmptyState />
        ) : (
          rows.map((row) => {
            const barWidth = maxCount > 0 ? (row.count / maxCount) * 100 : 0;
            const isSelected = selectedEvent === row.event_name;

            return (
              <button
                key={row.event_name}
                onClick={() => onSelectEvent(isSelected ? null : row.event_name)}
                className={cn(
                  'group relative w-full px-4 py-3 flex items-center gap-2 text-left',
                  'transition-colors duration-100',
                  isSelected
                    ? 'bg-spark/8 border-l-2 border-l-spark'
                    : 'hover:bg-surface-2/50 border-l-2 border-l-transparent'
                )}
              >
                {/* Relative bar chart background */}
                <div
                  className="absolute left-0 top-0 h-full bg-spark/5 pointer-events-none"
                  style={{ width: `${barWidth}%` }}
                />
                <span className="relative flex-1 text-sm text-ink font-medium truncate">
                  {row.event_name}
                </span>
                <span className="relative w-20 text-right text-sm font-mono tabular-nums text-ink-2">
                  {row.visitors.toLocaleString()}
                </span>
                <span className="relative w-20 text-right text-sm font-mono tabular-nums text-ink">
                  {row.count.toLocaleString()}
                </span>
                <span className="relative w-16 flex justify-end">
                  <DeltaBadge current={row.count} prev={row.prev_count} />
                </span>
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}
