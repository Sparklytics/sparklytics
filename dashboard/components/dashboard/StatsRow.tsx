'use client';

import { StatCard } from './StatCard';
import { formatNumber, formatDuration } from '@/lib/utils';
import type { StatsResponse, PageviewsPoint } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

interface StatsRowProps {
  stats?: StatsResponse;
  series?: PageviewsPoint[];
  loading: boolean;
}

function toSparklineData(series: PageviewsPoint[], key: 'pageviews' | 'visitors') {
  return series.map((p) => ({ date: p.date, value: p[key] }));
}

function computeDelta(current: number, prev: number): number | undefined {
  if (prev === 0) return undefined;
  return ((current - prev) / prev) * 100;
}

export function StatsRow({ stats, series = [], loading }: StatsRowProps) {
  const { compare } = useFilters();
  const compareActive = compare.mode !== 'none';
  const pageviewSpark = toSparklineData(series, 'pageviews');
  const visitorSpark = toSparklineData(series, 'visitors');

  const bounceDelta =
    compareActive && stats ? computeDelta(stats.bounce_rate, stats.prev_bounce_rate) : undefined;

  const pagesPerSession = stats && stats.sessions > 0 ? stats.pageviews / stats.sessions : 0;
  const prevPagesPerSession = stats && stats.prev_sessions > 0 ? stats.prev_pageviews / stats.prev_sessions : 0;

  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4">
      <StatCard
        label="Pageviews"
        value={stats ? formatNumber(stats.pageviews) : '—'}
        delta={compareActive && stats ? computeDelta(stats.pageviews, stats.prev_pageviews) : undefined}
        sparklineData={pageviewSpark}
        loading={loading}
      />
      <StatCard
        label="Visitors"
        value={stats ? formatNumber(stats.visitors) : '—'}
        delta={compareActive && stats ? computeDelta(stats.visitors, stats.prev_visitors) : undefined}
        sparklineData={visitorSpark}
        loading={loading}
      />
      <StatCard
        label="Sessions"
        value={stats ? formatNumber(stats.sessions) : '—'}
        delta={compareActive && stats ? computeDelta(stats.sessions, stats.prev_sessions) : undefined}
        loading={loading}
      />
      <StatCard
        label="Bounce Rate"
        value={stats ? `${(stats.bounce_rate * 100).toFixed(0)}%` : '—'}
        delta={bounceDelta !== undefined ? -bounceDelta : undefined}
        loading={loading}
      />
      <StatCard
        label="Avg. Duration"
        value={stats ? formatDuration(stats.avg_duration_seconds) : '—'}
        delta={compareActive && stats ? computeDelta(stats.avg_duration_seconds, stats.prev_avg_duration_seconds) : undefined}
        loading={loading}
      />
      <StatCard
        label="Pages/Session"
        value={stats ? pagesPerSession.toFixed(1) : '—'}
        delta={compareActive && stats ? computeDelta(pagesPerSession, prevPagesPerSession) : undefined}
        loading={loading}
      />
    </div>
  );
}
