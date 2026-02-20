'use client';

import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { useState } from 'react';
import { Skeleton } from '@/components/ui/skeleton';
import { cn, formatNumber } from '@/lib/utils';
import type { PageviewsPoint } from '@/lib/api';

interface PageviewsChartProps {
  data?: PageviewsPoint[];
  loading: boolean;
}

function CustomTooltip({ active, payload, label }: any) {
  if (!active || !payload?.length) return null;

  let dateStr = label;
  try {
    dateStr = new Date(label).toLocaleDateString(undefined, {
      weekday: 'short',
      month: 'short',
      day: 'numeric',
      year: 'numeric'
    });
  } catch (e) { }

  return (
    <div className="bg-surface-1 border border-line rounded p-3 text-xs min-w-[150px]">
      <div className="text-ink font-medium mb-3">{dateStr}</div>
      <div className="flex flex-col gap-2">
        {payload.map((p: any) => (
          <div key={p.name} className="flex justify-between items-center tabular-nums">
            <div className="flex items-center gap-1.5 text-ink-3 capitalize">
              <span className="w-1.5 h-1.5 rounded-full" style={{ background: p.color }} />
              {p.name}
            </div>
            <span className="font-medium text-ink">{formatNumber(p.value)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

export function PageviewsChart({ data, loading }: PageviewsChartProps) {
  const [metric, setMetric] = useState<'both' | 'visitors' | 'pageviews'>('both');

  if (loading) {
    return (
      <div className="bg-surface-1 border border-line rounded-lg p-6">
        <div className="flex items-center justify-between mb-6">
          <Skeleton className="h-4 w-24 bg-surface-2" />
          <Skeleton className="h-8 w-[180px] bg-surface-2 rounded" />
        </div>
        <Skeleton className="h-[240px] w-full bg-surface-2" />
      </div>
    );
  }

  return (
    <div className="bg-surface-1 border border-line rounded-lg p-6">
      {/* Header with metric toggle */}
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-sm font-medium text-ink">Traffic Overview</h2>

        <div className="flex items-center gap-1 bg-surface-2 p-1 rounded border border-line">
          <button
            onClick={() => setMetric('both')}
            className={cn("px-2.5 py-1 text-xs rounded transition-colors", metric === 'both' ? 'bg-surface-1 text-ink border border-line' : 'text-ink-3 hover:text-ink-2 border border-transparent')}
          >
            All
          </button>
          <button
            onClick={() => setMetric('visitors')}
            className={cn("px-2.5 py-1 text-xs rounded transition-colors", metric === 'visitors' ? 'bg-surface-1 text-ink border border-line' : 'text-ink-3 hover:text-ink-2 border border-transparent')}
          >
            Visitors
          </button>
          <button
            onClick={() => setMetric('pageviews')}
            className={cn("px-2.5 py-1 text-xs rounded transition-colors", metric === 'pageviews' ? 'bg-surface-1 text-ink border border-line' : 'text-ink-3 hover:text-ink-2 border border-transparent')}
          >
            Pageviews
          </button>
        </div>
      </div>

      <ResponsiveContainer width="100%" height={240}>
        <LineChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
          <CartesianGrid stroke="var(--line)" strokeDasharray="3 3" vertical={false} />
          <XAxis
            dataKey="date"
            tick={{ fill: 'var(--ink-3)', fontSize: 11 }}
            tickLine={false}
            axisLine={false}
            tickFormatter={(v: string) => {
              const d = new Date(v);
              return `${d.getMonth() + 1}/${d.getDate()}`;
            }}
          />
          <YAxis
            tick={{ fill: 'var(--ink-3)', fontSize: 11 }}
            tickLine={false}
            axisLine={false}
            tickFormatter={(v: number) => formatNumber(v)}
          />
          <Tooltip content={<CustomTooltip />} cursor={{ stroke: 'var(--line)', strokeWidth: 1, strokeDasharray: '4 4' }} />
          {(metric === 'both' || metric === 'visitors') && (
            <Line
              type="monotone"
              dataKey="visitors"
              stroke="var(--spark)"
              strokeWidth={2}
              dot={false}
              isAnimationActive={false}
            />
          )}
          {(metric === 'both' || metric === 'pageviews') && (
            <Line
              type="monotone"
              dataKey="pageviews"
              stroke="var(--neutral)"
              strokeWidth={2}
              dot={false}
              isAnimationActive={false}
            />
          )}
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
