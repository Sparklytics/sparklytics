'use client';

import { useEffect, useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { useCreateReport, useUpdateReport } from '@/hooks/useReports';
import { usePreviewReport } from '@/hooks/useReportRun';
import type {
  DateRangeType,
  ReportConfig,
  ReportRunResult,
  ReportType,
  SavedReport,
} from '@/lib/api';

interface ReportFormDialogProps {
  websiteId: string;
  open: boolean;
  onClose: () => void;
  editingReportId?: string | null;
  editingReport?: SavedReport | null;
  onPreview: (title: string, result: ReportRunResult) => void;
}

const inputClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-3 focus:outline-none focus:ring-1 focus:ring-spark focus:border-spark disabled:opacity-50 disabled:cursor-not-allowed';

const selectClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink focus:outline-none focus:ring-1 focus:ring-spark focus:border-spark disabled:opacity-50 disabled:cursor-not-allowed';

const labelClass = 'block text-xs font-medium text-ink-3 mb-1';

function defaultConfig(): ReportConfig {
  return {
    version: 1,
    report_type: 'stats',
    date_range_type: 'relative',
    relative_days: 30,
    compare_mode: 'none',
    timezone: 'UTC',
  };
}

function normalizeConfig(config: ReportConfig): ReportConfig {
  const timezone = config.timezone?.trim() || 'UTC';
  const reportType: ReportType = config.report_type;
  const dateRangeType: DateRangeType = config.date_range_type;

  return {
    ...config,
    version: 1,
    report_type: reportType,
    date_range_type: dateRangeType,
    timezone,
    metric_type: reportType === 'metrics' ? config.metric_type : undefined,
    relative_days: dateRangeType === 'relative' ? config.relative_days : undefined,
    start_date: dateRangeType === 'absolute' ? config.start_date : undefined,
    end_date: dateRangeType === 'absolute' ? config.end_date : undefined,
    compare_mode: config.compare_mode ?? 'none',
    compare_start_date:
      config.compare_mode === 'custom' ? config.compare_start_date : undefined,
    compare_end_date:
      config.compare_mode === 'custom' ? config.compare_end_date : undefined,
  };
}

export function ReportFormDialog({
  websiteId,
  open,
  onClose,
  editingReportId,
  editingReport,
  onPreview,
}: ReportFormDialogProps) {
  const isEditing = !!editingReportId;
  const isEditingLoading = isEditing && !editingReport;
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [config, setConfig] = useState<ReportConfig>(defaultConfig());
  const [apiError, setApiError] = useState<string | null>(null);

  const createReport = useCreateReport(websiteId);
  const updateReport = useUpdateReport(websiteId);
  const previewReport = usePreviewReport(websiteId);

  useEffect(() => {
    if (!open) return;
    setApiError(null);
    if (isEditingLoading) {
      setName('');
      setDescription('');
      setConfig(defaultConfig());
      return;
    }
    if (isEditing && editingReport) {
      setName(editingReport.name);
      setDescription(editingReport.description ?? '');
      setConfig(editingReport.config);
      return;
    }
    setName('');
    setDescription('');
    setConfig(defaultConfig());
  }, [open, editingReport, isEditing, isEditingLoading]);

  const isPending = createReport.isPending || updateReport.isPending;

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (previewReport.isPending) {
      setApiError('Please wait for preview to finish before saving.');
      return;
    }
    setApiError(null);
    const normalized = normalizeConfig(config);
    const payload = {
      name: name.trim(),
      description: description.trim() ? description.trim() : null,
      config: normalized,
    };

    if (isEditing && editingReport) {
      updateReport.mutate(
        {
          reportId: editingReport.id,
          payload,
        },
        {
          onSuccess: () => onClose(),
          onError: (err) => setApiError(err.message),
        }
      );
      return;
    }

    createReport.mutate(payload, {
      onSuccess: () => onClose(),
      onError: (err) => setApiError(err.message),
    });
  }

  function handlePreview() {
    setApiError(null);
    const normalized = normalizeConfig(config);
    previewReport.mutate(normalized, {
      onSuccess: (data) => {
        onPreview(`Preview: ${name.trim() || 'Untitled report'}`, data.data);
      },
      onError: (err) => setApiError(err.message),
    });
  }

  return (
    <Dialog open={open} onOpenChange={(value) => { if (!value) onClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-xl max-w-lg max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-base font-semibold text-ink">
            {isEditing ? 'Edit report' : 'New report'}
          </DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="report-name" className={labelClass}>Name</label>
            <input
              id="report-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className={inputClass}
              maxLength={100}
              placeholder="e.g. Weekly KPI"
              required
            />
          </div>

          <div>
            <label htmlFor="report-description" className={labelClass}>Description</label>
            <input
              id="report-description"
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className={inputClass}
              placeholder="Optional"
              maxLength={200}
            />
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label htmlFor="report-type" className={labelClass}>Report type</label>
              <select
                id="report-type"
                value={config.report_type}
                onChange={(e) =>
                  setConfig((prev) => {
                    const nextType = e.target.value as ReportType;
                    return {
                      ...prev,
                      report_type: nextType,
                      metric_type:
                        nextType === 'metrics' ? prev.metric_type ?? 'page' : undefined,
                    };
                  })
                }
                className={selectClass}
              >
                <option value="stats">Stats</option>
                <option value="pageviews">Pageviews</option>
                <option value="metrics">Metrics</option>
                <option value="events">Events</option>
              </select>
            </div>
            <div>
              <label htmlFor="report-timezone" className={labelClass}>Timezone</label>
              <input
                id="report-timezone"
                type="text"
                value={config.timezone ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, timezone: e.target.value }))}
                className={inputClass}
                placeholder="UTC"
              />
            </div>
          </div>

          {config.report_type === 'metrics' && (
            <div>
              <label htmlFor="report-metric-type" className={labelClass}>Metric type</label>
              <select
                id="report-metric-type"
                value={config.metric_type ?? 'page'}
                onChange={(e) => setConfig((prev) => ({ ...prev, metric_type: e.target.value }))}
                className={selectClass}
              >
                <option value="page">Page</option>
                <option value="referrer">Referrer</option>
                <option value="country">Country</option>
                <option value="region">Region</option>
                <option value="city">City</option>
                <option value="browser">Browser</option>
                <option value="os">OS</option>
                <option value="device">Device</option>
                <option value="event_name">Event name</option>
                <option value="utm_source">UTM source</option>
                <option value="utm_medium">UTM medium</option>
                <option value="utm_campaign">UTM campaign</option>
              </select>
            </div>
          )}

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label htmlFor="report-date-range-type" className={labelClass}>Date range type</label>
              <select
                id="report-date-range-type"
                value={config.date_range_type}
                onChange={(e) => setConfig((prev) => ({ ...prev, date_range_type: e.target.value as DateRangeType }))}
                className={selectClass}
              >
                <option value="relative">Relative</option>
                <option value="absolute">Absolute</option>
              </select>
            </div>

            {config.date_range_type === 'relative' ? (
              <div>
                <label htmlFor="report-relative-days" className={labelClass}>Relative days</label>
                <input
                  id="report-relative-days"
                  type="number"
                  min={1}
                  max={365}
                  value={config.relative_days ?? 30}
                  onChange={(e) =>
                    setConfig((prev) => ({ ...prev, relative_days: Number(e.target.value) }))
                  }
                  className={inputClass}
                />
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label htmlFor="report-start-date" className={labelClass}>Start date</label>
                  <input
                    id="report-start-date"
                    type="date"
                    value={config.start_date ?? ''}
                    onChange={(e) => setConfig((prev) => ({ ...prev, start_date: e.target.value }))}
                    className={inputClass}
                  />
                </div>
                <div>
                  <label htmlFor="report-end-date" className={labelClass}>End date</label>
                  <input
                    id="report-end-date"
                    type="date"
                    value={config.end_date ?? ''}
                    onChange={(e) => setConfig((prev) => ({ ...prev, end_date: e.target.value }))}
                    className={inputClass}
                  />
                </div>
              </div>
            )}
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label htmlFor="report-compare-mode" className={labelClass}>Compare</label>
              <select
                id="report-compare-mode"
                value={config.compare_mode ?? 'none'}
                onChange={(e) =>
                  setConfig((prev) => {
                    const mode = e.target.value as typeof prev.compare_mode;
                    if (mode === 'custom') {
                      return {
                        ...prev,
                        compare_mode: mode,
                        compare_start_date: prev.compare_start_date ?? prev.start_date,
                        compare_end_date: prev.compare_end_date ?? prev.end_date,
                      };
                    }
                    return {
                      ...prev,
                      compare_mode: mode,
                      compare_start_date: undefined,
                      compare_end_date: undefined,
                    };
                  })
                }
                className={selectClass}
              >
                <option value="none">No compare</option>
                <option value="previous_period">Previous period</option>
                <option value="previous_year">Previous year</option>
                <option value="custom">Custom</option>
              </select>
            </div>
            {config.compare_mode === 'custom' && (
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label htmlFor="report-compare-start" className={labelClass}>Compare start</label>
                  <input
                    id="report-compare-start"
                    type="date"
                    value={config.compare_start_date ?? ''}
                    onChange={(e) =>
                      setConfig((prev) => ({ ...prev, compare_start_date: e.target.value || undefined }))
                    }
                    className={inputClass}
                  />
                </div>
                <div>
                  <label htmlFor="report-compare-end" className={labelClass}>Compare end</label>
                  <input
                    id="report-compare-end"
                    type="date"
                    value={config.compare_end_date ?? ''}
                    onChange={(e) =>
                      setConfig((prev) => ({ ...prev, compare_end_date: e.target.value || undefined }))
                    }
                    className={inputClass}
                  />
                </div>
              </div>
            )}
          </div>

          <div className="space-y-2">
            <p className="text-xs font-medium text-ink-3">Filters</p>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              <input
                type="text"
                aria-label="Country"
                value={config.filter_country ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_country: e.target.value || undefined }))}
                className={inputClass}
                placeholder="Country (e.g. US)"
              />
              <input
                type="text"
                aria-label="Browser"
                value={config.filter_browser ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_browser: e.target.value || undefined }))}
                className={inputClass}
                placeholder="Browser"
              />
              <input
                type="text"
                aria-label="OS"
                value={config.filter_os ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_os: e.target.value || undefined }))}
                className={inputClass}
                placeholder="OS"
              />
              <input
                type="text"
                aria-label="Device"
                value={config.filter_device ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_device: e.target.value || undefined }))}
                className={inputClass}
                placeholder="Device"
              />
              <input
                type="text"
                aria-label="Page contains"
                value={config.filter_page ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_page: e.target.value || undefined }))}
                className={inputClass}
                placeholder="Page contains"
              />
              <input
                type="text"
                aria-label="Referrer"
                value={config.filter_referrer ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_referrer: e.target.value || undefined }))}
                className={inputClass}
                placeholder="Referrer"
              />
              <input
                type="text"
                aria-label="UTM source"
                value={config.filter_utm_source ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_utm_source: e.target.value || undefined }))}
                className={inputClass}
                placeholder="UTM source"
              />
              <input
                type="text"
                aria-label="UTM medium"
                value={config.filter_utm_medium ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_utm_medium: e.target.value || undefined }))}
                className={inputClass}
                placeholder="UTM medium"
              />
              <input
                type="text"
                aria-label="UTM campaign"
                value={config.filter_utm_campaign ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_utm_campaign: e.target.value || undefined }))}
                className={inputClass}
                placeholder="UTM campaign"
              />
              <input
                type="text"
                aria-label="Region"
                value={config.filter_region ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_region: e.target.value || undefined }))}
                className={inputClass}
                placeholder="Region"
              />
              <input
                type="text"
                aria-label="City"
                value={config.filter_city ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_city: e.target.value || undefined }))}
                className={inputClass}
                placeholder="City"
              />
              <input
                type="text"
                aria-label="Hostname"
                value={config.filter_hostname ?? ''}
                onChange={(e) => setConfig((prev) => ({ ...prev, filter_hostname: e.target.value || undefined }))}
                className={inputClass}
                placeholder="Hostname"
              />
            </div>
          </div>

          {apiError && (
            <p className="text-xs text-red-400 border border-red-400/20 bg-red-400/5 rounded-md px-3 py-2">
              {apiError}
            </p>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onClose}
              className="text-xs"
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handlePreview}
              disabled={previewReport.isPending || isPending || isEditingLoading}
              className="text-xs"
            >
              {previewReport.isPending ? 'Previewing…' : 'Preview'}
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={isPending || previewReport.isPending || isEditingLoading}
              className="text-xs"
            >
              {isPending ? 'Saving…' : isEditing ? 'Save changes' : 'Create report'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
