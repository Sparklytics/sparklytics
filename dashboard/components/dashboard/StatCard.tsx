'use client';

import { TrendingUp, TrendingDown } from 'lucide-react';
import { Sparkline } from './Sparkline';
import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';

interface StatCardProps {
  label: string;
  value: string;
  delta?: number;
  sparklineData?: { date: string; value: number }[];
  loading?: boolean;
}

function TrendBadge({ delta }: { delta: number }) {
  const isUp = delta > 0;
  const isDown = delta < 0;
  const sign = isUp ? '+' : '';

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 text-[11px] px-1.5 py-0.5 rounded-[4px] font-medium tabular-nums',
        isUp && 'text-up bg-up/10',
        isDown && 'text-down bg-down/10',
        !isUp && !isDown && 'text-ink-3 bg-surface-2'
      )}
    >
      {isUp && <TrendingUp className="w-2.5 h-2.5" />}
      {isDown && <TrendingDown className="w-2.5 h-2.5" />}
      {sign}{Math.abs(delta).toFixed(0)}%
    </span>
  );
}

export function StatCard({ label, value, delta, sparklineData, loading }: StatCardProps) {
  if (loading) {
    return (
      <div className="bg-surface-1 border border-line rounded-lg p-4 flex flex-col gap-3">
        <Skeleton className="h-2.5 w-14 bg-surface-2" />
        <Skeleton className="h-9 w-20 bg-surface-2" />
        <Skeleton className="h-[32px] w-full bg-surface-2" />
      </div>
    );
  }

  return (
    <div className="bg-surface-1 border border-line rounded-lg p-4 flex flex-col gap-0 relative overflow-hidden">
      <div className="flex items-start justify-between gap-2 mb-2">
        <span className="text-[11px] text-ink-3 uppercase tracking-[0.07em] font-medium leading-none pt-px">
          {label}
        </span>
        {delta !== undefined && <TrendBadge delta={delta} />}
      </div>
      <span className="text-[34px] font-semibold tracking-tight tabular-nums text-ink leading-none">
        {value}
      </span>
      {sparklineData && sparklineData.length > 0 && (
        <div className="mt-4">
          <Sparkline
            data={sparklineData}
            color={delta !== undefined && delta < 0 ? 'var(--down)' : 'var(--spark)'}
          />
        </div>
      )}
    </div>
  );
}
