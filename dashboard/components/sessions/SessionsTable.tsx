'use client';

import { cn, formatDuration } from '@/lib/utils';
import type { SessionListItem } from '@/lib/api';

interface SessionsTableProps {
  sessions: SessionListItem[];
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  isFetching: boolean;
  fetchNextPage: () => void;
  selectedSessionId: string | null;
  onSelect: (id: string) => void;
}

function SkeletonRow() {
  return (
    <tr className="animate-pulse border-b border-line">
      {Array.from({ length: 7 }).map((_, i) => (
        <td key={i} className="px-3 py-3">
          <div className="h-4 bg-surface-2 rounded" style={{ width: i === 0 ? '120px' : i === 4 ? '160px' : '60px' }} />
        </td>
      ))}
    </tr>
  );
}

function parseDisplayUrl(url: string | null): string {
  if (!url) return '—';
  try {
    const u = new URL(url);
    return u.pathname || '/';
  } catch {
    return url;
  }
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts.replace(' ', 'T'));
    const diffMin = Math.floor((Date.now() - d.getTime()) / 60000);
    if (diffMin < 1) return 'just now';
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffH = Math.floor(diffMin / 60);
    if (diffH < 24) return `${diffH}h ago`;
    const diffD = Math.floor(diffH / 24);
    if (diffD < 7) return `${diffD}d ago`;
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return ts.slice(0, 16);
  }
}

export function SessionsTable({
  sessions,
  hasNextPage,
  isFetchingNextPage,
  isFetching,
  fetchNextPage,
  selectedSessionId,
  onSelect,
}: SessionsTableProps) {
  const isLoading = isFetching && sessions.length === 0;

  return (
    <div className="border border-line rounded-lg bg-surface-1">
      {/* Header */}
      <div className="px-4 py-3 border-b border-line flex items-center justify-between">
        <h3 className="text-sm font-medium text-ink">Sessions</h3>
        {!isLoading && sessions.length > 0 && (
          <span className="text-xs text-ink-3 font-mono tabular-nums">
            {sessions.length.toLocaleString()} loaded
          </span>
        )}
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-xs font-medium text-ink-3 uppercase tracking-wider">
              <th className="px-3 py-2 text-left">Started</th>
              <th className="px-3 py-2 text-right">Duration</th>
              <th className="px-3 py-2 text-right">Pages</th>
              <th className="px-3 py-2 text-right">Events</th>
              <th className="px-3 py-2 text-left">Entry</th>
              <th className="px-3 py-2 text-left">Country</th>
              <th className="px-3 py-2 text-left">Browser</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {isLoading ? (
              Array.from({ length: 10 }).map((_, i) => <SkeletonRow key={i} />)
            ) : sessions.length === 0 ? (
              <tr>
                <td colSpan={7} className="px-4 py-12 text-center text-sm text-ink-3">
                  No sessions found for the selected date range.
                </td>
              </tr>
            ) : (
              sessions.map((session) => {
                const isSelected = selectedSessionId === session.session_id;
                return (
                  <tr
                    key={session.session_id}
                    onClick={() => onSelect(session.session_id)}
                    className={cn(
                      'cursor-pointer transition-colors duration-100',
                      isSelected
                        ? 'bg-spark/8 border-l-2 border-l-spark'
                        : 'hover:bg-surface-2/50 border-l-2 border-l-transparent'
                    )}
                  >
                    <td className="px-3 py-3 font-mono tabular-nums text-xs text-ink-2 whitespace-nowrap">
                      {formatTime(session.first_seen)}
                    </td>
                    <td className="px-3 py-3 font-mono tabular-nums text-xs text-right text-ink-2 whitespace-nowrap">
                      {formatDuration(session.duration_seconds)}
                    </td>
                    <td className="px-3 py-3 font-mono tabular-nums text-xs text-right text-ink-2">
                      {session.pageview_count}
                    </td>
                    <td className="px-3 py-3 font-mono tabular-nums text-xs text-right text-ink-2">
                      {session.event_count}
                    </td>
                    <td className="px-3 py-3 text-xs text-ink max-w-[200px] truncate">
                      {parseDisplayUrl(session.entry_page)}
                    </td>
                    <td className="px-3 py-3 text-xs text-ink-2">
                      {session.country ?? '—'}
                    </td>
                    <td className="px-3 py-3 text-xs text-ink-2">
                      {session.browser ?? '—'}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {hasNextPage && (
        <div className="px-4 py-3 border-t border-line">
          <button
            onClick={fetchNextPage}
            disabled={isFetchingNextPage}
            className="w-full text-xs text-ink-3 hover:text-ink py-1.5 rounded-md border border-line hover:bg-surface-2 transition-colors disabled:opacity-50"
          >
            {isFetchingNextPage ? 'Loading…' : 'Load 50 more'}
          </button>
        </div>
      )}
    </div>
  );
}
