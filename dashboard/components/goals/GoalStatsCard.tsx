'use client';

import { cn } from '@/lib/utils';
import type { GoalStats } from '@/lib/api';

interface GoalStatsCardProps {
  stats: GoalStats | undefined;
  loading?: boolean;
  variant?: 'compact' | 'full';
}

function TrendBadge({ trendPct }: { trendPct: number | null }) {
  if (trendPct === null) return null;
  const up = trendPct >= 0;
  return (
    <span
      className={cn(
        'text-xs font-mono tabular-nums px-1 rounded-sm',
        up ? 'text-spark bg-spark/10' : 'text-down bg-down/10'
      )}
    >
      {up ? '▲' : '▼'} {Math.abs(trendPct).toFixed(1)}%
    </span>
  );
}

export function GoalStatsCard({ stats, loading, variant = 'compact' }: GoalStatsCardProps) {
  if (loading) {
    return <div className="h-5 w-20 bg-surface-2 rounded animate-pulse" />;
  }

  if (!stats) {
    return <span className="text-xs text-ink-3">—</span>;
  }

  const rateStr = (stats.conversion_rate * 100).toFixed(2) + '%';

  if (variant === 'compact') {
    return (
      <div className="flex items-center gap-2">
        <span className="font-mono tabular-nums text-sm text-ink">{rateStr}</span>
        <TrendBadge trendPct={stats.trend_pct} />
      </div>
    );
  }

  // Full variant
  return (
    <div className="space-y-1">
      <div className="flex items-baseline gap-2">
        <span className="font-mono tabular-nums text-2xl font-semibold text-ink">{rateStr}</span>
        <TrendBadge trendPct={stats.trend_pct} />
      </div>
      <p className="text-xs text-ink-3 font-mono tabular-nums">
        {stats.conversions.toLocaleString()} events · {stats.converting_sessions.toLocaleString()}/{stats.total_sessions.toLocaleString()} sessions
      </p>
    </div>
  );
}
