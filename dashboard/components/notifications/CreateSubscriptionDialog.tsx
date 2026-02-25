'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { useReports } from '@/hooks/useReports';
import { useCreateReportSubscription } from '@/hooks/useNotifications';
import type { NotificationChannel, SubscriptionSchedule } from '@/lib/api';

interface CreateSubscriptionDialogProps {
  websiteId: string;
}

export function CreateSubscriptionDialog({ websiteId }: CreateSubscriptionDialogProps) {
  const { data: reportsData } = useReports(websiteId);
  const createSubscription = useCreateReportSubscription(websiteId);
  const reports = reportsData?.data ?? [];
  const [open, setOpen] = useState(false);
  const [reportId, setReportId] = useState('');
  const [schedule, setSchedule] = useState<SubscriptionSchedule>('daily');
  const [timezone, setTimezone] = useState('UTC');
  const [channel, setChannel] = useState<NotificationChannel>('email');
  const [target, setTarget] = useState('');

  const canSubmit = reportId.trim() && target.trim();

  return (
    <div className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-sm font-medium text-ink">Create subscription</p>
          <p className="text-xs text-ink-3">Schedule saved reports to email or webhook.</p>
        </div>
        <Button type="button" size="sm" onClick={() => setOpen((v) => !v)}>
          {open ? 'Close' : 'New subscription'}
        </Button>
      </div>
      {open && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Report</span>
            <select
              aria-label="Report"
              value={reportId}
              onChange={(e) => setReportId(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
            >
              <option value="">Select report</option>
              {reports.map((report) => (
                <option key={report.id} value={report.id}>{report.name}</option>
              ))}
            </select>
          </label>
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Schedule</span>
            <select
              aria-label="Schedule"
              value={schedule}
              onChange={(e) => setSchedule(e.target.value as SubscriptionSchedule)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
            >
              <option value="daily">Daily</option>
              <option value="weekly">Weekly</option>
              <option value="monthly">Monthly</option>
            </select>
          </label>
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Timezone</span>
            <input
              aria-label="Timezone"
              value={timezone}
              onChange={(e) => setTimezone(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
              placeholder="UTC"
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Channel</span>
            <select
              aria-label="Channel"
              value={channel}
              onChange={(e) => setChannel(e.target.value as NotificationChannel)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
            >
              <option value="email">Email</option>
              <option value="webhook">Webhook</option>
            </select>
          </label>
          <label className="space-y-1 md:col-span-2">
            <span className="text-xs text-ink-2">{channel === 'email' ? 'Email target' : 'Webhook target'}</span>
            <input
              aria-label={channel === 'email' ? 'Email target' : 'Webhook target'}
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
              placeholder={channel === 'email' ? 'team@example.com' : 'https://hooks.example.com/endpoint'}
            />
          </label>
          <div className="md:col-span-2">
            <Button
              type="button"
              size="sm"
              disabled={!canSubmit || createSubscription.isPending}
              onClick={() => {
                createSubscription.mutate(
                  {
                    report_id: reportId,
                    schedule,
                    timezone: timezone.trim() || 'UTC',
                    channel,
                    target: target.trim(),
                  },
                  {
                    onSuccess: () => {
                      setTarget('');
                      setOpen(false);
                    },
                  }
                );
              }}
            >
              {createSubscription.isPending ? 'Creating…' : 'Create subscription'}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
