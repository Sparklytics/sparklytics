'use client';

import { useState } from 'react';
import { useSessions } from '@/hooks/useSessions';
import { useStats } from '@/hooks/useStats';
import { formatNumber, formatDuration } from '@/lib/utils';
import { SessionsTable } from './SessionsTable';
import { SessionDetailDrawer } from './SessionDetailDrawer';

interface SessionsPageProps {
  websiteId: string;
}

export function SessionsPage({ websiteId }: SessionsPageProps) {
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);

  const {
    data,
    isFetching,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
  } = useSessions(websiteId);

  const { data: statsData, isLoading: statsLoading } = useStats(websiteId);
  const stats = statsData?.data;

  const sessions = data?.pages.flatMap((p) => p.data) ?? [];

  const summaryCards = [
    {
      label: 'Sessions',
      value: stats ? formatNumber(stats.sessions) : null,
    },
    {
      label: 'Avg. Duration',
      value: stats ? formatDuration(stats.avg_duration_seconds) : null,
    },
    {
      label: 'Bounce Rate',
      value: stats ? `${stats.bounce_rate.toFixed(1)}%` : null,
    },
    {
      label: 'Visitors',
      value: stats ? formatNumber(stats.visitors) : null,
    },
  ];

  return (
    <div>
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-6">
        {summaryCards.map((card) => (
          <div
            key={card.label}
            className="border border-line rounded-lg bg-surface-1 p-4"
          >
            <p className="text-[11px] text-ink-3 uppercase tracking-[0.07em] font-medium">
              {card.label}
            </p>
            {statsLoading || card.value === null ? (
              <div className="mt-1 h-8 w-20 animate-pulse bg-surface-2 rounded" />
            ) : (
              <p className="text-2xl font-mono font-semibold tabular-nums text-ink mt-1">
                {card.value}
              </p>
            )}
          </div>
        ))}
      </div>

      <SessionsTable
        sessions={sessions}
        hasNextPage={!!hasNextPage}
        isFetchingNextPage={isFetchingNextPage}
        isFetching={isFetching}
        fetchNextPage={fetchNextPage}
        selectedSessionId={selectedSessionId}
        onSelect={setSelectedSessionId}
      />

      <SessionDetailDrawer
        websiteId={websiteId}
        sessionId={selectedSessionId}
        onClose={() => setSelectedSessionId(null)}
      />
    </div>
  );
}
