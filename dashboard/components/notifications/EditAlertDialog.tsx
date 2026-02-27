'use client';

import { useState, useEffect } from 'react';
import { Loader2 } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import type { AlertRule, AlertMetric, AlertConditionType, NotificationChannel } from '@/lib/api';

const inputClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-4 focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

const selectClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

const labelClass = 'block text-xs font-medium text-ink-3 mb-1';

const METRICS: AlertMetric[] = ['pageviews', 'visitors', 'conversions', 'conversion_rate'];
const CONDITIONS: AlertConditionType[] = ['spike', 'drop', 'threshold_above', 'threshold_below'];
const CHANNELS: NotificationChannel[] = ['email', 'webhook'];

interface EditAlertDialogProps {
  rule: AlertRule | null;
  isPending: boolean;
  onSave: (alertId: string, payload: {
    metric: AlertMetric;
    condition_type: AlertConditionType;
    threshold_value: number;
    channel: NotificationChannel;
    target: string;
  }) => void;
  onClose: () => void;
}

export function EditAlertDialog({ rule, isPending, onSave, onClose }: EditAlertDialogProps) {
  const [metric, setMetric] = useState<AlertMetric>('pageviews');
  const [conditionType, setConditionType] = useState<AlertConditionType>('spike');
  const [thresholdValue, setThresholdValue] = useState('');
  const [channel, setChannel] = useState<NotificationChannel>('email');
  const [target, setTarget] = useState('');

  useEffect(() => {
    if (rule) {
      setMetric(rule.metric);
      setConditionType(rule.condition_type);
      setThresholdValue(String(rule.threshold_value));
      setChannel(rule.channel);
      setTarget(rule.target);
    }
  }, [rule]);

  function handleSubmit() {
    if (!rule || !target.trim()) return;
    const threshold = Number(thresholdValue);
    if (isNaN(threshold)) return;
    onSave(rule.id, {
      metric,
      condition_type: conditionType,
      threshold_value: threshold,
      channel,
      target: target.trim(),
    });
  }

  return (
    <Dialog open={!!rule} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-lg max-w-md">
        <DialogHeader>
          <DialogTitle className="text-ink">Edit Alert Rule</DialogTitle>
          <DialogDescription className="sr-only">Edit the alert rule configuration</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <label className="block">
            <span className={labelClass}>Metric</span>
            <select value={metric} onChange={(e) => setMetric(e.target.value as AlertMetric)} className={selectClass}>
              {METRICS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Condition</span>
            <select value={conditionType} onChange={(e) => setConditionType(e.target.value as AlertConditionType)} className={selectClass}>
              {CONDITIONS.map((c) => <option key={c} value={c}>{c.replace('_', ' ')}</option>)}
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Threshold Value</span>
            <input
              type="number"
              value={thresholdValue}
              onChange={(e) => setThresholdValue(e.target.value)}
              placeholder="e.g. 100"
              className={inputClass}
            />
          </label>

          <label className="block">
            <span className={labelClass}>Channel</span>
            <select value={channel} onChange={(e) => setChannel(e.target.value as NotificationChannel)} className={selectClass}>
              {CHANNELS.map((ch) => <option key={ch} value={ch}>{ch}</option>)}
            </select>
          </label>

          <label className="block">
            <span className={labelClass}>Target</span>
            <input
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              placeholder={channel === 'email' ? 'user@example.com' : 'https://hooks.example.com/...'}
              className={inputClass}
            />
          </label>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button onClick={handleSubmit} disabled={isPending || !target.trim() || isNaN(Number(thresholdValue))}>
            {isPending && <Loader2 className="w-4 h-4 animate-spin" />}
            Save Changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
