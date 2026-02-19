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
import { Skeleton } from '@/components/ui/skeleton';
import { formatNumber } from '@/lib/utils';
import type { PageviewsPoint } from '@/lib/api';

interface PageviewsChartProps {
  data?: PageviewsPoint[];
  loading: boolean;
}

function CustomTooltip({ active, payload, label }: {
  active?: boolean;
  payload?: { value: number; name: string; color: string }[];
  label?: string;
}) {
  if (!active || !payload?.length) return null;
  return (
    <div className="bg-surface-2 border border-line-3 rounded px-3 py-2 text-xs">
      <p className="text-ink-3 mb-1">{label}</p>
      {payload.map((p) => (
        <p key={p.name} className="font-mono tabular-nums" style={{ color: p.color }}>
          {p.name}: {formatNumber(p.value)}
        </p>
      ))}
    </div>
  );
}

export function PageviewsChart({ data, loading }: PageviewsChartProps) {
  if (loading) {
    return (
      <div className="bg-surface-1 border border-line rounded-lg p-6">
        <div className="flex items-center gap-4 mb-6">
          <Skeleton className="h-4 w-24 bg-surface-2" />
          <div className="flex items-center gap-3 ml-auto">
            <Skeleton className="h-3 w-20 bg-surface-2" />
            <Skeleton className="h-3 w-20 bg-surface-2" />
          </div>
        </div>
        <Skeleton className="h-[240px] w-full bg-surface-2" />
      </div>
    );
  }

  return (
    <div className="bg-surface-1 border border-line rounded-lg p-6">
      {/* Header with legend dots */}
      <div className="flex items-center gap-4 mb-6">
        <h2 className="text-sm font-medium text-ink">Traffic Overview</h2>
        <div className="flex items-center gap-4 ml-auto">
          <span className="flex items-center gap-2 text-xs text-ink-3">
            <span className="w-2 h-2 rounded-full bg-spark" />
            Visitors
          </span>
          <span className="flex items-center gap-2 text-xs text-ink-3">
            <span className="w-2 h-2 rounded-full bg-neutral" />
            Pageviews
          </span>
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
          <Tooltip content={<CustomTooltip />} />
          <Line
            type="monotone"
            dataKey="visitors"
            stroke="var(--spark)"
            strokeWidth={2}
            dot={false}
            isAnimationActive
            animationDuration={600}
          />
          <Line
            type="monotone"
            dataKey="pageviews"
            stroke="var(--neutral)"
            strokeWidth={2}
            dot={false}
            isAnimationActive
            animationDuration={600}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
