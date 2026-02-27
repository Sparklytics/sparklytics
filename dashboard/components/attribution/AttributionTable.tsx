'use client';

import { AttributionRow } from '@/lib/api';

interface AttributionTableProps {
  rows: AttributionRow[];
  loading: boolean;
}

function formatPct(value: number) {
  return `${(value * 100).toFixed(1)}%`;
}

function formatMoney(value: number) {
  return value.toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export function AttributionTable({ rows, loading }: AttributionTableProps) {
  if (loading) {
    return (
      <div className="border border-line rounded-lg bg-surface-1 divide-y divide-line">
        {Array.from({ length: 5 }).map((_, index) => (
          <div key={index} className="px-4 py-3 animate-pulse flex items-center gap-4">
            <div className="h-4 bg-surface-2 rounded w-28" />
            <div className="flex-1" />
            <div className="h-4 bg-surface-2 rounded w-16" />
            <div className="h-4 bg-surface-2 rounded w-16" />
            <div className="h-4 bg-surface-2 rounded w-12" />
          </div>
        ))}
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className="border border-line rounded-lg bg-surface-1 px-6 py-10 text-center">
        <p className="text-sm font-medium text-ink mb-1">No attributed conversions</p>
        <p className="text-xs text-ink-3">Try widening the date range or checking goal match rules.</p>
      </div>
    );
  }

  const maxConversions = Math.max(...rows.map((r) => r.conversions), 1);

  return (
    <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-xs font-medium text-ink-3 uppercase tracking-wider">
              <th className="px-4 py-2 text-left">Channel</th>
              <th className="px-4 py-2 text-right">Conversions</th>
              <th className="px-4 py-2 text-right">Revenue</th>
              <th className="px-4 py-2 text-right">Share</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const barPct = Math.round((row.conversions / maxConversions) * 100);
              return (
                <tr key={row.channel} className="border-b border-line/70 last:border-b-0 relative">
                  <td className="px-4 py-3 text-ink font-medium relative">
                    <div
                      className="absolute inset-y-0 left-0 rounded-r-sm"
                      style={{ width: `${barPct}%`, background: 'var(--spark-subtle)' }}
                    />
                    <span className="relative">{row.channel}</span>
                  </td>
                  <td className="px-4 py-3 text-right font-mono tabular-nums text-ink-2">
                    {row.conversions.toLocaleString()}
                  </td>
                  <td className="px-4 py-3 text-right font-mono tabular-nums text-ink-2">
                    {formatMoney(row.revenue)}
                  </td>
                  <td className="px-4 py-3 text-right font-mono tabular-nums text-ink-2">
                    {formatPct(row.share)}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
