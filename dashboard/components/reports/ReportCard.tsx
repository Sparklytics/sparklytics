'use client';

import { Play, Pencil, Trash2 } from 'lucide-react';
import type { SavedReportSummary } from '@/lib/api';
import { ReportTypeBadge } from './ReportTypeBadge';

interface ReportCardProps {
  report: SavedReportSummary;
  isRunning: boolean;
  onRun: (reportId: string) => void;
  onEdit: (reportId: string) => void;
  onDelete: (reportId: string) => void;
}

function formatDate(value: string | null): string {
  if (!value) return 'Never';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export function ReportCard({ report, isRunning, onRun, onEdit, onDelete }: ReportCardProps) {
  return (
    <div className="border border-line rounded-lg bg-surface-1 px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1 min-w-0">
          <div className="flex items-center gap-2">
            <p className="text-sm font-medium text-ink truncate">{report.name}</p>
            <ReportTypeBadge type={report.report_type} />
          </div>
          <p className="text-xs text-ink-3">
            {report.description?.trim() ? report.description : 'No description'}
          </p>
          <p className="text-xs text-ink-3 font-mono tabular-nums">
            Last run: {formatDate(report.last_run_at)}
          </p>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => onRun(report.id)}
            disabled={isRunning}
            aria-label="Run report"
            className="p-1.5 text-ink-3 hover:text-ink hover:bg-surface-2 rounded-md transition-colors disabled:opacity-50"
            title="Run report"
          >
            <Play className="w-3.5 h-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onEdit(report.id)}
            aria-label="Edit report"
            className="p-1.5 text-ink-3 hover:text-ink hover:bg-surface-2 rounded-md transition-colors"
            title="Edit report"
          >
            <Pencil className="w-3.5 h-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onDelete(report.id)}
            aria-label="Delete report"
            className="p-1.5 text-ink-3 hover:text-red-400 hover:bg-red-400/10 rounded-md transition-colors"
            title="Delete report"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  );
}
