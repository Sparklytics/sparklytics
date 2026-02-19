'use client';

import { StatCard } from './StatCard';
import { formatNumber, formatDuration } from '@/lib/utils';
import type { StatsResponse, PageviewsPoint } from '@/lib/api';

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
  const pageviewSpark = toSparklineData(series, 'pageviews');
  const visitorSpark = toSparklineData(series, 'visitors');

  const bounceDelta = stats ? computeDelta(stats.bounce_rate, stats.prev_bounce_rate) : undefined;

  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
      <StatCard
        label="Pageviews"
        value={stats ? formatNumber(stats.pageviews) : '—'}
        delta={stats ? computeDelta(stats.pageviews, stats.prev_pageviews) : undefined}
        sparklineData={pageviewSpark}
        loading={loading}
      />
      <StatCard
        label="Visitors"
        value={stats ? formatNumber(stats.visitors) : '—'}
        delta={stats ? computeDelta(stats.visitors, stats.prev_visitors) : undefined}
        sparklineData={visitorSpark}
        loading={loading}
      />
      <StatCard
        label="Sessions"
        value={stats ? formatNumber(stats.sessions) : '—'}
        delta={stats ? computeDelta(stats.sessions, stats.prev_sessions) : undefined}
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
        delta={stats ? computeDelta(stats.avg_duration_seconds, stats.prev_avg_duration_seconds) : undefined}
        loading={loading}
      />
    </div>
  );
}
