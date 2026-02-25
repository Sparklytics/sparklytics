'use client';

import { useState } from 'react';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import type { ReportRunResult } from '@/lib/api';
import { useDeleteReport, useReport, useReports } from '@/hooks/useReports';
import { useRunReport } from '@/hooks/useReportRun';
import { ReportCard } from './ReportCard';
import { ReportFormDialog } from './ReportFormDialog';
import { ReportResultPanel } from './ReportResultPanel';

interface ReportsPageProps {
  websiteId: string;
}

export function ReportsPage({ websiteId }: ReportsPageProps) {
  const { data, isLoading } = useReports(websiteId);
  const deleteReport = useDeleteReport(websiteId);
  const runReport = useRunReport(websiteId);
  const reports = data?.data ?? [];

  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [editingReportId, setEditingReportId] = useState<string | null>(null);
  const [deletingReportId, setDeletingReportId] = useState<string | null>(null);
  const [resultTitle, setResultTitle] = useState('Report output');
  const [result, setResult] = useState<ReportRunResult | null>(null);
  const { data: editingReportData } = useReport(websiteId, editingReportId);
  const editingReport = editingReportData?.data ?? null;

  function handleRun(reportId: string) {
    const report = reports.find((r) => r.id === reportId);
    runReport.mutate(reportId, {
      onSuccess: (data) => {
        setResult(data.data);
        setResultTitle(report ? `Run: ${report.name}` : 'Run result');
      },
    });
  }

  function handleDeleteConfirm() {
    if (!deletingReportId) return;
    deleteReport.mutate(deletingReportId, {
      onSuccess: () => {
        setDeletingReportId(null);
      },
    });
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-ink">Reports</h2>
          <p className="text-xs text-ink-3 mt-0.5">
            Save reusable analytics views and run them on demand.
          </p>
        </div>
        <Button
          size="sm"
          onClick={() => setCreateDialogOpen(true)}
          className="text-xs gap-1"
        >
          <Plus className="w-3.5 h-3.5" />
          New Report
        </Button>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[380px_1fr] gap-4 items-start">
        <div className="space-y-2">
          {isLoading ? (
            <div className="border border-line rounded-lg bg-surface-1 divide-y divide-line">
              {Array.from({ length: 3 }).map((_, index) => (
                <div key={index} className="px-4 py-4 animate-pulse">
                  <div className="h-4 bg-surface-2 rounded w-2/3" />
                </div>
              ))}
            </div>
          ) : reports.length === 0 ? (
            <div className="border border-line rounded-lg bg-surface-1 px-6 py-10 text-center">
              <p className="text-sm font-medium text-ink mb-1">No reports yet</p>
              <p className="text-xs text-ink-3">Create your first saved report.</p>
            </div>
          ) : (
            reports.map((report) => (
              <ReportCard
                key={report.id}
                report={report}
                isRunning={runReport.isPending}
                onRun={handleRun}
                onEdit={setEditingReportId}
                onDelete={setDeletingReportId}
              />
            ))
          )}
        </div>

        <ReportResultPanel
          result={result}
          title={resultTitle}
          isPending={runReport.isPending}
        />
      </div>

      <ReportFormDialog
        websiteId={websiteId}
        open={createDialogOpen || !!editingReport}
        onClose={() => {
          setCreateDialogOpen(false);
          setEditingReportId(null);
        }}
        editingReport={editingReport}
        onPreview={(title, previewResult) => {
          setResultTitle(title);
          setResult(previewResult);
        }}
      />

      <ConfirmDialog
        open={!!deletingReportId}
        onOpenChange={(open) => {
          if (!open) setDeletingReportId(null);
        }}
        title="Delete report?"
        description="This action cannot be undone."
        confirmLabel="Delete"
        destructive
        loading={deleteReport.isPending}
        onConfirm={handleDeleteConfirm}
      />
    </div>
  );
}
