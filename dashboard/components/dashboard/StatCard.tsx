'use client';

import { TrendingUp, TrendingDown, Minus } from 'lucide-react';
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
        'inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-sm font-mono tabular-nums',
        isUp && 'bg-up/10 text-up',
        isDown && 'bg-down/10 text-down',
        !isUp && !isDown && 'bg-surface-2 text-ink-3'
      )}
    >
      {isUp && <TrendingUp className="w-3 h-3" />}
      {isDown && <TrendingDown className="w-3 h-3" />}
      {!isUp && !isDown && <Minus className="w-3 h-3" />}
      {sign}{delta.toFixed(0)}%
    </span>
  );
}

export function StatCard({ label, value, delta, sparklineData, loading }: StatCardProps) {
  if (loading) {
    return (
      <div className="bg-surface-1 border border-line rounded-lg p-4 flex flex-col gap-3">
        <Skeleton className="h-3 w-16 bg-surface-2" />
        <Skeleton className="h-8 w-24 bg-surface-2" />
        <Skeleton className="h-3 w-12 bg-surface-2" />
        <Skeleton className="h-[30px] w-full bg-surface-2" />
      </div>
    );
  }

  return (
    <div className="bg-surface-1 border border-line rounded-lg p-4 flex flex-col gap-2">
      <span className="text-xs text-ink-2 uppercase tracking-wide">{label}</span>
      <span className="font-mono tabular-nums text-3xl leading-none text-ink">
        {value}
      </span>
      {delta !== undefined && <TrendBadge delta={delta} />}
      {sparklineData && sparklineData.length > 0 && (
        <div className="mt-1">
          <Sparkline
            data={sparklineData}
            color={delta !== undefined && delta < 0 ? 'var(--down)' : 'var(--spark)'}
          />
        </div>
      )}
    </div>
  );
}
