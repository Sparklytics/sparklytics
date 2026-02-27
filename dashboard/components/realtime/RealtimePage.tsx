'use client';

import { FileText, Target } from 'lucide-react';
import { useRealtime } from '@/hooks/useRealtime';
import { Skeleton } from '@/components/ui/skeleton';
import { formatNumber } from '@/lib/utils';

interface RealtimePageProps {
  websiteId: string;
}

function timeAgo(isoString: string): string {
  const diff = Date.now() - new Date(isoString).getTime();
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  return `${Math.floor(m / 60)}h ago`;
}

function navigate(path: string) {
  window.history.pushState({}, '', path);
  window.dispatchEvent(new PopStateEvent('popstate'));
}

export function RealtimePage({ websiteId }: RealtimePageProps) {
  const { data: realtimeData, isLoading } = useRealtime(websiteId, 10_000);
  const data = realtimeData?.data;

  if (isLoading) {
    return (
      <div className="space-y-6">
        <div className="bg-surface-1 border border-line rounded-lg p-8 flex items-center justify-center">
          <Skeleton className="h-12 w-24 bg-surface-2" />
        </div>
        <div className="bg-surface-1 border border-line rounded-lg p-6">
          {[...Array(5)].map((_, i) => (
            <Skeleton key={i} className="h-8 w-full mb-2 bg-surface-2" />
          ))}
        </div>
      </div>
    );
  }

  const events = data?.recent_events ?? [];
  const pagination = data?.pagination;

  return (
    <div className="space-y-6">
      {/* Large visitor counter */}
      <div className="bg-surface-1 border border-line rounded-lg p-8 text-center">
        <div className="flex items-center justify-center gap-3 mb-2">
          <span className="relative flex w-3 h-3">
            <span className="animate-[pulse-spark_2s_ease-in-out_infinite] absolute inline-flex h-full w-full rounded-full bg-spark opacity-75" />
            <span className="relative inline-flex rounded-full w-3 h-3 bg-spark" />
          </span>
          <span className="text-sm text-ink-2">Live</span>
        </div>
        <p className="font-mono tabular-nums text-spark text-5xl font-semibold">
          {data ? formatNumber(data.active_visitors) : '0'}
        </p>
        <p className="text-sm text-ink-3 mt-2">
          {data?.active_visitors === 1 ? 'visitor' : 'visitors'} active in the last 30 minutes
        </p>
      </div>

      {/* Recent events */}
      <div className="bg-surface-1 border border-line rounded-lg">
        <div className="flex items-center justify-between px-6 py-4 border-b border-line">
          <div className="flex items-center gap-2">
            <span className="relative flex w-2 h-2">
              <span className="animate-[pulse-spark_2s_ease-in-out_infinite] absolute inline-flex h-full w-full rounded-full bg-spark opacity-75" />
              <span className="relative inline-flex rounded-full w-2 h-2 bg-spark" />
            </span>
            <span className="text-sm font-medium text-ink">Recent activity</span>
          </div>
          <span className="text-xs text-ink-3">Auto-refreshes every 10s</span>
        </div>

        {events.length === 0 ? (
          <div className="px-6 py-12 text-center">
            <p className="text-sm text-ink-3 mb-4">No activity in the last 30 minutes</p>
            <button
              onClick={() => navigate(`/dashboard/${websiteId}/settings/snippet`)}
              className="text-xs text-spark hover:underline"
            >
              View tracking snippet
            </button>
          </div>
        ) : (
          <div className="divide-y divide-line">
            {events.map((event, i) => (
              <div key={i} className="flex items-center gap-4 px-6 py-3">
                <span className="shrink-0 text-ink-3">
                  {event.event_type === 'pageview' ? <FileText className="w-4 h-4" /> : <Target className="w-4 h-4" />}
                </span>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-ink truncate">{event.url}</p>
                  {event.referrer_domain && (
                    <p className="text-xs text-ink-3 truncate">from {event.referrer_domain}</p>
                  )}
                </div>
                <div className="flex items-center gap-4 shrink-0">
                  {event.country && (
                    <span className="text-xs text-ink-3">{event.country}</span>
                  )}
                  {event.browser && (
                    <span className="text-xs text-ink-3 hidden md:inline">{event.browser}</span>
                  )}
                  <span className="text-xs text-ink-4 font-mono tabular-nums w-16 text-right">
                    {timeAgo(event.ts)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}

        {pagination && events.length > 0 && (
          <div className="px-6 py-3 border-t border-line">
            <p className="text-xs text-ink-4">
              Showing <span className="font-mono tabular-nums">{events.length}</span> of <span className="font-mono tabular-nums">{pagination.total_in_window}</span> events in last 30 minutes
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
