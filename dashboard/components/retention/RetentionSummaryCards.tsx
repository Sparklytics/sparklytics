'use client';

import { RetentionGranularity, RetentionSummary } from '@/lib/api';

interface RetentionSummaryCardsProps {
  granularity: RetentionGranularity;
  summary: RetentionSummary;
}

function periodLabel(granularity: RetentionGranularity, offset: number): string {
  if (granularity === 'day') return `Day ${offset}`;
  if (granularity === 'week') return `Week ${offset}`;
  return `Month ${offset}`;
}

function formatRate(rate: number | null): string {
  if (rate === null) return '—';
  return `${(rate * 100).toFixed(1)}%`;
}

export function RetentionSummaryCards({ granularity, summary }: RetentionSummaryCardsProps) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
      <div className="border border-line rounded-lg bg-surface-1 p-4">
        <p className="text-xs text-ink-3">{`Avg ${periodLabel(granularity, 1)} Retention`}</p>
        <p className="mt-1 font-mono tabular-nums text-lg text-ink">
          {formatRate(summary.avg_period1_rate)}
        </p>
      </div>
      <div className="border border-line rounded-lg bg-surface-1 p-4">
        <p className="text-xs text-ink-3">{`Avg ${periodLabel(granularity, 4)} Retention`}</p>
        <p className="mt-1 font-mono tabular-nums text-lg text-ink">
          {formatRate(summary.avg_period4_rate)}
        </p>
      </div>
    </div>
  );
}
