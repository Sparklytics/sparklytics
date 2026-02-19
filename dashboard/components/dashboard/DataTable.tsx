'use client';

import { useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { Skeleton } from '@/components/ui/skeleton';
import { cn, formatNumber } from '@/lib/utils';
import type { MetricRow } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

interface DataTableProps {
  title: string;
  filterKey: string;
  data?: MetricRow[];
  loading: boolean;
  showPageviews?: boolean;
}

export function DataTable({ title, filterKey, data = [], loading, showPageviews = false }: DataTableProps) {
  const [sortBy, setSortBy] = useState<'visitors' | 'pageviews'>('visitors');
  const { setFilter } = useFilters();

  const maxValue = Math.max(...data.map((r) => r.visitors), 1);

  const sorted = [...data].sort((a, b) =>
    sortBy === 'pageviews' ? (b.pageviews ?? 0) - (a.pageviews ?? 0) : b.visitors - a.visitors
  );

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
    <div className="bg-surface-1 border border-line rounded-lg p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-medium text-ink">{title}</h3>
        <div className="flex items-center gap-3 text-xs text-ink-3">
          <button
            onClick={() => setSortBy('visitors')}
            className={cn(
              'flex items-center gap-1 transition-colors',
              sortBy === 'visitors' ? 'text-ink' : 'hover:text-ink-2'
            )}
          >
            Visitors
            {sortBy === 'visitors' ? <ChevronDown className="w-3 h-3" /> : null}
          </button>
          {showPageviews && (
            <button
              onClick={() => setSortBy('pageviews')}
              className={cn(
                'flex items-center gap-1 transition-colors',
                sortBy === 'pageviews' ? 'text-ink' : 'hover:text-ink-2'
              )}
            >
              Pageviews
              {sortBy === 'pageviews' ? <ChevronDown className="w-3 h-3" /> : null}
            </button>
          )}
        </div>
      </div>

      {/* Rows */}
      <div>
        {sorted.length === 0 ? (
          <p className="text-xs text-ink-4 py-4 text-center">No data</p>
        ) : (
          sorted.slice(0, 10).map((row) => {
            const pct = Math.round((row.visitors / maxValue) * 100);
            return (
              <div
                key={row.value}
                className="relative flex items-center justify-between py-2 border-b border-line last:border-0 hover:bg-surface-2 -mx-2 px-2 cursor-pointer transition-colors rounded"
                onClick={() => setFilter(filterKey, row.value)}
              >
                {/* Background bar */}
                <div
                  className="absolute inset-y-0 left-0 rounded"
                  style={{
                    width: `${pct}%`,
                    background: 'var(--spark-subtle)',
                  }}
                />
                <span className="relative text-xs text-ink truncate max-w-[60%]">{row.value || '(direct)'}</span>
                <span className="relative font-mono tabular-nums text-xs text-ink-2">
                  {sortBy === 'pageviews' && row.pageviews !== undefined
                    ? formatNumber(row.pageviews)
                    : formatNumber(row.visitors)}
                </span>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
