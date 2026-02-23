'use client';

import { useState } from 'react';
import { ChevronDown, Monitor, Smartphone, Tablet, Globe, Laptop, MapPin } from 'lucide-react';
import { Skeleton } from '@/components/ui/skeleton';
import { cn, formatNumber, formatDuration } from '@/lib/utils';
import type { MetricRow } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

interface DataTableProps {
  title: string;
  filterKey: string;
  data?: MetricRow[];
  loading: boolean;
  showPageviews?: boolean;
  totalVisitors?: number;
}

function formatRowLabel(filterKey: string, value: string): string {
  if (!value) return '(direct)';
  if (filterKey === 'page') {
    try {
      const u = new URL(value);
      return u.pathname + (u.search || '');
    } catch {
      return value;
    }
  }
  return value;
}

function getRowIcon(filterKey: string, value: string) {
  if (filterKey === 'device') {
    if (value === 'mobile') return <Smartphone className="w-3.5 h-3.5 text-ink-3 shrink-0" />;
    if (value === 'tablet') return <Tablet className="w-3.5 h-3.5 text-ink-3 shrink-0" />;
    return <Monitor className="w-3.5 h-3.5 text-ink-3 shrink-0" />;
  }
  if (filterKey === 'browser') {
    return <Globe className="w-3.5 h-3.5 text-ink-3 shrink-0" />;
  }
  if (filterKey === 'os') {
    if (['iOS', 'Android'].includes(value)) return <Smartphone className="w-3.5 h-3.5 text-ink-3 shrink-0" />;
    return <Laptop className="w-3.5 h-3.5 text-ink-3 shrink-0" />;
  }
  if (filterKey === 'country') {
    return <MapPin className="w-3.5 h-3.5 text-ink-3 shrink-0" />;
  }
  return null;
}

type SortKey = 'visitors' | 'pageviews' | 'bounce_rate' | 'avg_duration_seconds';

export function DataTable({ title, filterKey, data = [], loading, showPageviews = false, totalVisitors }: DataTableProps) {
  const [sortBy, setSortBy] = useState<SortKey>('visitors');
  const { setFilter } = useFilters();

  const maxValue = Math.max(...data.map((r) => r.visitors), 1);

  const sorted = [...data].sort((a, b) => {
    if (sortBy === 'pageviews') return (b.pageviews ?? 0) - (a.pageviews ?? 0);
    if (sortBy === 'bounce_rate') return b.bounce_rate - a.bounce_rate;
    if (sortBy === 'avg_duration_seconds') return b.avg_duration_seconds - a.avg_duration_seconds;
    return b.visitors - a.visitors;
  });

  if (loading) {
    return (
      <div className="bg-surface-1 border border-line rounded-lg p-6">
        <Skeleton className="h-4 w-24 mb-4 bg-surface-2" />
        {[...Array(5)].map((_, i) => (
          <div key={i} className="flex items-center gap-4 py-2 border-b border-line last:border-0">
            <Skeleton className="h-3 flex-1 bg-surface-2" />
            <Skeleton className="h-3 w-12 bg-surface-2" />
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="bg-surface-1 border border-line rounded-xl p-5">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-[13px] font-medium text-ink">{title}</h3>
        <div className="flex items-center gap-3 text-[11px] text-ink-3">
          {(['visitors', ...(showPageviews ? ['pageviews'] : []), 'bounce_rate', 'avg_duration_seconds'] as SortKey[]).map((key) => {
            const label = key === 'visitors' ? 'Visitors'
              : key === 'pageviews' ? 'Pageviews'
              : key === 'bounce_rate' ? 'Bounce'
              : 'Duration';
            return (
              <button
                key={key}
                onClick={() => setSortBy(key)}
                className={cn(
                  'flex items-center gap-1 transition-colors',
                  sortBy === key ? 'text-ink' : 'hover:text-ink-2'
                )}
              >
                {label}
                {sortBy === key ? <ChevronDown className="w-3 h-3" /> : null}
              </button>
            );
          })}
        </div>
      </div>

      {/* Rows */}
      <div>
        {sorted.length === 0 ? (
          <p className="text-[12px] text-ink-4 py-6 text-center">No data</p>
        ) : (
          sorted.slice(0, 10).map((row) => {
            const pct = Math.round((row.visitors / maxValue) * 100);
            const sharePct = totalVisitors && totalVisitors > 0
              ? Math.round((row.visitors / totalVisitors) * 100)
              : null;
            return (
              <div
                key={row.value}
                className="relative flex items-center justify-between py-2 border-b border-line last:border-0 hover:bg-white/[0.03] -mx-1.5 px-1.5 cursor-pointer transition-colors rounded-md"
                onClick={() => setFilter(filterKey, row.value)}
              >
                {/* Background bar */}
                <div
                  className="absolute inset-y-0 left-0 rounded-md"
                  style={{
                    width: `${pct}%`,
                    background: 'var(--spark-subtle)',
                  }}
                />
                <div className="relative flex items-center gap-2 max-w-[60%]">
                  {getRowIcon(filterKey, row.value)}
                  <span className="text-[12px] text-ink truncate">{formatRowLabel(filterKey, row.value)}</span>
                </div>
                <div className="relative flex items-center gap-2 font-mono tabular-nums text-[12px] text-ink-3">
                  {sortBy === 'pageviews' && row.pageviews !== undefined
                    ? formatNumber(row.pageviews)
                    : sortBy === 'bounce_rate'
                    ? `${(row.bounce_rate ?? 0).toFixed(1)}%`
                    : sortBy === 'avg_duration_seconds'
                    ? (row.avg_duration_seconds > 0 ? formatDuration(row.avg_duration_seconds) : '—')
                    : formatNumber(row.visitors)}
                  {sortBy === 'visitors' && sharePct !== null && (
                    <span className="text-[10px] text-ink-4 w-7 text-right">{sharePct}%</span>
                  )}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
