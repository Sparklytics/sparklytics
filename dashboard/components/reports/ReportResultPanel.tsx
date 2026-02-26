'use client';

import type { ReportRunResult } from '@/lib/api';
import { ReportTypeBadge } from './ReportTypeBadge';

interface ReportResultPanelProps {
  result: ReportRunResult | null;
  title: string;
  isPending: boolean;
}

function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

export function ReportResultPanel({ result, title, isPending }: ReportResultPanelProps) {
  if (!result) {
    return (
      <div className="border border-line rounded-lg bg-surface-1 px-6 py-10 text-center">
        <p className="text-sm font-medium text-ink mb-1">No report result yet</p>
        <p className="text-xs text-ink-3">Run or preview a report to see output here.</p>
      </div>
    );
  }

  const compare = (result.data as { compare?: { mode?: string; primary_range?: [string, string]; comparison_range?: [string, string] } })
    .compare;

  return (
    <div className={`border border-line rounded-lg bg-surface-1 transition-opacity ${isPending ? 'opacity-60' : ''}`}>
      <div className="flex items-center justify-between gap-3 px-4 py-3 border-b border-line">
        <div className="min-w-0">
          <p className="text-sm font-medium text-ink truncate">{title}</p>
          <p className="text-xs text-ink-3 font-mono tabular-nums">{formatDateTime(result.ran_at)}</p>
          {compare?.mode && (
            <p className="text-[11px] text-ink-4 font-mono tabular-nums mt-1">
              {compare.mode}: {compare.primary_range?.join(' → ')} vs {compare.comparison_range?.join(' → ')}
            </p>
          )}
        </div>
        <ReportTypeBadge type={result.config.report_type} />
      </div>
      <pre className="p-4 text-xs text-ink-2 font-mono tabular-nums overflow-auto max-h-[480px]">
        {JSON.stringify(result.data, null, 2)}
      </pre>
    </div>
  );
}
