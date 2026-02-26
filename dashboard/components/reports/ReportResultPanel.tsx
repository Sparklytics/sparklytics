'use client';

import type { ReportRunResult } from '@/lib/api';
import { ReportTypeBadge } from './ReportTypeBadge';

interface ReportResultPanelProps {
  result: ReportRunResult | null;
  title: string;
  isPending: boolean;
}

type CompareSummary = {
  mode: string;
  primaryRange: [string, string];
  comparisonRange: [string, string];
};

function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function isStringRange(value: unknown): value is [string, string] {
  return (
    Array.isArray(value) &&
    value.length === 2 &&
    typeof value[0] === 'string' &&
    typeof value[1] === 'string'
  );
}

function extractCompareSummary(data: unknown): CompareSummary | null {
  if (!data || typeof data !== 'object') {
    return null;
  }

  const compare = (data as { compare?: unknown }).compare;
  if (!compare || typeof compare !== 'object') {
    return null;
  }

  const mode = (compare as { mode?: unknown }).mode;
  const primaryRange = (compare as { primary_range?: unknown }).primary_range;
  const comparisonRange = (compare as { comparison_range?: unknown }).comparison_range;

  if (typeof mode !== 'string' || mode.trim().length === 0) {
    return null;
  }
  if (!isStringRange(primaryRange) || !isStringRange(comparisonRange)) {
    return null;
  }

  return {
    mode,
    primaryRange,
    comparisonRange,
  };
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

  const compare = extractCompareSummary(result.data);

  return (
    <div className={`border border-line rounded-lg bg-surface-1 transition-opacity ${isPending ? 'opacity-60' : ''}`}>
      <div className="flex items-center justify-between gap-3 px-4 py-3 border-b border-line">
        <div className="min-w-0">
          <p className="text-sm font-medium text-ink truncate">{title}</p>
          <p className="text-xs text-ink-3 font-mono tabular-nums">{formatDateTime(result.ran_at)}</p>
          {compare && (
            <p className="text-[11px] text-ink-4 font-mono tabular-nums mt-1">
              {compare.mode}: {compare.primaryRange.join(' → ')} vs {compare.comparisonRange.join(' → ')}
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
