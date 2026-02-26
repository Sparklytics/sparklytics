'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  useCreateAlertRule,
} from '@/hooks/useNotifications';
import type { AlertConditionType, AlertMetric, NotificationChannel } from '@/lib/api';

interface CreateAlertDialogProps {
  websiteId: string;
}

export function CreateAlertDialog({ websiteId }: CreateAlertDialogProps) {
  const createAlert = useCreateAlertRule(websiteId);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const [metric, setMetric] = useState<AlertMetric>('pageviews');
  const [conditionType, setConditionType] = useState<AlertConditionType>('spike');
  const [thresholdValue, setThresholdValue] = useState('2');
  const [lookbackDays, setLookbackDays] = useState('7');
  const [channel, setChannel] = useState<NotificationChannel>('email');
  const [target, setTarget] = useState('');

  const canSubmit = name.trim() && target.trim();

  return (
    <div className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-sm font-medium text-ink">Create alert rule</p>
          <p className="text-xs text-ink-3">Trigger notifications on spikes, drops, or thresholds.</p>
        </div>
        <Button type="button" size="sm" onClick={() => setOpen((v) => !v)}>
          {open ? 'Close' : 'New alert'}
        </Button>
      </div>
      {open && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <label className="space-y-1 md:col-span-2">
            <span className="text-xs text-ink-2">Name</span>
            <input
              aria-label="Name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
              placeholder="Traffic spike alert"
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Metric</span>
            <select
              aria-label="Metric"
              value={metric}
              onChange={(e) => setMetric(e.target.value as AlertMetric)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
            >
              <option value="pageviews">Pageviews</option>
              <option value="visitors">Visitors</option>
              <option value="conversions">Conversions</option>
              <option value="conversion_rate">Conversion rate</option>
            </select>
          </label>
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Condition</span>
            <select
              aria-label="Condition"
              value={conditionType}
              onChange={(e) => setConditionType(e.target.value as AlertConditionType)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
            >
              <option value="spike">Spike</option>
              <option value="drop">Drop</option>
              <option value="threshold_above">Threshold above</option>
              <option value="threshold_below">Threshold below</option>
            </select>
          </label>
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Threshold</span>
            <input
              aria-label="Threshold"
              value={thresholdValue}
              onChange={(e) => setThresholdValue(e.target.value)}
              type="number"
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-ink-2">Lookback days</span>
            <input
              aria-label="Lookback days"
              value={lookbackDays}
              onChange={(e) => setLookbackDays(e.target.value)}
              type="number"
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
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
          <label className="space-y-1">
            <span className="text-xs text-ink-2">{channel === 'email' ? 'Email target' : 'Webhook target'}</span>
            <input
              aria-label={channel === 'email' ? 'Email target' : 'Webhook target'}
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-2 py-2 text-sm text-ink"
            />
          </label>
          <div className="md:col-span-2">
            <Button
              type="button"
              size="sm"
              disabled={!canSubmit || createAlert.isPending}
              onClick={() => {
                createAlert.mutate(
                  {
                    name: name.trim(),
                    metric,
                    condition_type: conditionType,
                    threshold_value: Number(thresholdValue),
                    lookback_days: Number(lookbackDays),
                    channel,
                    target: target.trim(),
                  },
                  {
                    onSuccess: () => {
                      setOpen(false);
                      setName('');
                      setTarget('');
                    },
                  }
                );
              }}
            >
              {createAlert.isPending ? 'Creating…' : 'Create alert'}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
