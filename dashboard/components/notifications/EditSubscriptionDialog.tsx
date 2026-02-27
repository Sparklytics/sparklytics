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
import { TIMEZONE_GROUPS } from '@/lib/timezones';
import type { ReportSubscription, SubscriptionSchedule, NotificationChannel } from '@/lib/api';

const inputClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-4 focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

const selectClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

const labelClass = 'block text-xs font-medium text-ink-3 mb-1';

const SCHEDULES: SubscriptionSchedule[] = ['daily', 'weekly', 'monthly'];
const CHANNELS: NotificationChannel[] = ['email', 'webhook'];

interface EditSubscriptionDialogProps {
  subscription: ReportSubscription | null;
  isPending: boolean;
  onSave: (subscriptionId: string, payload: {
    schedule: SubscriptionSchedule;
    channel: NotificationChannel;
    target: string;
    timezone: string;
  }) => void;
  onClose: () => void;
}

export function EditSubscriptionDialog({ subscription, isPending, onSave, onClose }: EditSubscriptionDialogProps) {
  const [schedule, setSchedule] = useState<SubscriptionSchedule>('daily');
  const [channel, setChannel] = useState<NotificationChannel>('email');
  const [target, setTarget] = useState('');
  const [timezone, setTimezone] = useState('');

  useEffect(() => {
    if (subscription) {
      setSchedule(subscription.schedule);
      setChannel(subscription.channel);
      setTarget(subscription.target);
      setTimezone(subscription.timezone);
    }
  }, [subscription]);

  function handleSubmit() {
    if (!subscription || !target.trim()) return;
    onSave(subscription.id, {
      schedule,
      channel,
      target: target.trim(),
      timezone,
    });
  }

  return (
    <Dialog open={!!subscription} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-lg max-w-md">
        <DialogHeader>
          <DialogTitle className="text-ink">Edit Subscription</DialogTitle>
          <DialogDescription className="sr-only">Edit the report subscription settings</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <label className="block">
            <span className={labelClass}>Schedule</span>
            <select value={schedule} onChange={(e) => setSchedule(e.target.value as SubscriptionSchedule)} className={selectClass}>
              {SCHEDULES.map((s) => <option key={s} value={s}>{s}</option>)}
            </select>
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

          <label className="block">
            <span className={labelClass}>Timezone</span>
            <select value={timezone} onChange={(e) => setTimezone(e.target.value)} className={selectClass}>
              {Object.entries(TIMEZONE_GROUPS).map(([group, zones]) => (
                <optgroup key={group} label={group}>
                  {zones.map((tz) => (
                    <option key={tz} value={tz}>{tz}</option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button onClick={handleSubmit} disabled={isPending || !target.trim()}>
            {isPending && <Loader2 className="w-4 h-4 animate-spin" />}
            Save Changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
