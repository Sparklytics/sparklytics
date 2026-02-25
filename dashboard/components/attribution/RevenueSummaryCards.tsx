'use client';

import { RevenueSummary } from '@/lib/api';

interface RevenueSummaryCardsProps {
  summary: RevenueSummary;
  goalName?: string;
}

function formatMoney(value: number) {
  return value.toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export function RevenueSummaryCards({ summary, goalName }: RevenueSummaryCardsProps) {
  const modelLabel = summary.model === 'first_touch' ? 'First touch' : 'Last touch';

  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
      <div className="border border-line rounded-lg bg-surface-1 p-3">
        <p className="text-xs text-ink-3">Goal</p>
        <p data-testid="attribution-goal-value" className="mt-1 text-sm text-ink font-medium truncate">
          {goalName ?? summary.goal_id}
        </p>
      </div>
      <div className="border border-line rounded-lg bg-surface-1 p-3">
        <p className="text-xs text-ink-3">Conversions</p>
        <p data-testid="attribution-conversions-value" className="mt-1 text-lg text-ink font-mono tabular-nums">
          {summary.conversions.toLocaleString()}
        </p>
      </div>
      <div className="border border-line rounded-lg bg-surface-1 p-3">
        <p className="text-xs text-ink-3">Revenue ({modelLabel})</p>
        <p data-testid="attribution-revenue-value" className="mt-1 text-lg text-ink font-mono tabular-nums">
          {formatMoney(summary.revenue)}
        </p>
      </div>
    </div>
  );
}
