'use client';

import { Skeleton } from '@/components/ui/skeleton';
import { formatNumber } from '@/lib/utils';
import type { RealtimeResponse } from '@/lib/api';

interface RealtimePanelProps {
  data?: RealtimeResponse;
  loading: boolean;
}

function timeAgo(isoString: string): string {
  const seconds = Math.floor((Date.now() - new Date(isoString).getTime()) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  return `${Math.floor(seconds / 60)}m ago`;
}

export function RealtimePanel({ data, loading }: RealtimePanelProps) {
  if (loading) {
    return (
      <div className="bg-surface-1 border border-line rounded-lg p-6">
        <div className="flex items-center gap-2 mb-4">
          <Skeleton className="w-2 h-2 rounded-full bg-surface-2" />
          <Skeleton className="h-4 w-32 bg-surface-2" />
        </div>
        {[...Array(4)].map((_, i) => (
          <Skeleton key={i} className="h-8 w-full mb-2 bg-surface-2" />
        ))}
      </div>
    );
  }

  return (
    <div className="bg-surface-1 border border-line rounded-lg p-6">
      {/* Live indicator */}
      <div className="flex items-center gap-3 mb-4">
        <span className="relative flex w-2 h-2">
          <span className="animate-[pulse-spark_2s_ease-in-out_infinite] absolute inline-flex h-full w-full rounded-full bg-spark opacity-75" />
          <span className="relative inline-flex rounded-full w-2 h-2 bg-spark" />
        </span>
        <span className="text-sm font-medium text-ink">
          <span className="font-mono tabular-nums text-spark">
            {data ? formatNumber(data.active_visitors) : '0'}
          </span>
          <span className="text-ink-2 ml-2">
            {data?.active_visitors === 1 ? 'visitor' : 'visitors'} right now
          </span>
        </span>
      </div>

      {/* Recent events */}
      <div>
        {!data?.recent_events?.length && (
          <p className="text-xs text-ink-4 py-2 text-center">No recent activity</p>
        )}
        {data?.recent_events?.map((event, i) => (
          <div
            key={i}
            className="flex items-start gap-3 py-2 border-b border-line last:border-0"
          >
            <div className="flex-1 min-w-0">
              <p className="text-xs text-ink truncate">{event.url}</p>
              {event.referrer_domain && (
                <p className="text-xs text-ink-3 truncate">from {event.referrer_domain}</p>
              )}
            </div>
            <span className="text-xs text-ink-4 font-mono tabular-nums shrink-0">
              {timeAgo(event.ts)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
