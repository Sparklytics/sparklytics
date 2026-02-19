'use client';

import { useUsage } from '@/hooks/useUsage';
import { IS_CLOUD } from '@/lib/config';

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

export function UsageBadge() {
  const { data: usage } = useUsage();

  if (!IS_CLOUD || !usage) return null;

  const pct = Math.min(100, usage.percent_used);
  const isWarning = pct >= 80;
  const isCritical = pct >= 95;

  return (
    <div className="px-4 py-3 border-t border-line">
      <div className="flex items-center justify-between text-xs mb-2">
        <span className="text-ink-3">Events this month</span>
        <span className={`font-mono tabular-nums ${isCritical ? 'text-down' : isWarning ? 'text-warn' : 'text-ink-2'}`}>
          {formatCount(usage.event_count)}/{formatCount(usage.event_limit)}
        </span>
      </div>
      <div className="h-1 bg-surface-2 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-300 ${
            isCritical ? 'bg-down' : isWarning ? 'bg-warn' : 'bg-spark'
          }`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}
