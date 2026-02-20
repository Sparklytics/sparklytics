'use client';

import type { EventPropertyRow } from '@/lib/api';

interface EventPropertyTableProps {
  properties: EventPropertyRow[];
}

export function EventPropertyTable({ properties }: EventPropertyTableProps) {
  // Group rows by property_key for section display.
  const grouped: Record<string, EventPropertyRow[]> = {};
  for (const row of properties) {
    if (!grouped[row.property_key]) grouped[row.property_key] = [];
    grouped[row.property_key].push(row);
  }

  // Total count per key for relative bar width within that key.
  const keyTotals: Record<string, number> = {};
  for (const [key, rows] of Object.entries(grouped)) {
    keyTotals[key] = rows.reduce((s, r) => s + r.count, 0);
  }

  return (
    <div className="divide-y divide-line">
      {Object.entries(grouped).map(([key, rows]) => (
        <div key={key} className="px-4 py-3">
          <h5 className="text-xs font-semibold text-ink-3 uppercase tracking-wider mb-2">
            {key}
          </h5>
          <div className="space-y-1">
            {rows.map((row) => {
              const pct =
                keyTotals[key] > 0
                  ? Math.round((row.count / keyTotals[key]) * 100)
                  : 0;
              return (
                <div
                  key={`${row.property_key}:${row.property_value}`}
                  className="relative flex items-center gap-2 py-1"
                >
                  <div
                    className="absolute left-0 top-0 h-full bg-spark/10 rounded-sm pointer-events-none"
                    style={{ width: `${pct}%` }}
                  />
                  <span className="relative flex-1 text-sm text-ink truncate">
                    {row.property_value}
                  </span>
                  <span className="relative text-xs font-mono tabular-nums text-ink-3">
                    {row.count.toLocaleString()}
                  </span>
                  <span className="relative text-xs font-mono tabular-nums text-ink-3 w-8 text-right">
                    {pct}%
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}
