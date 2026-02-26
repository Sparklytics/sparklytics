'use client';

import { FlaskConical, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useDeleteReportSubscription, useReportSubscriptions, useTestReportSubscription, useUpdateReportSubscription } from '@/hooks/useNotifications';
import type { NotificationChannel, ReportSubscription, SubscriptionSchedule } from '@/lib/api';

interface SubscriptionsTableProps {
  websiteId: string;
}

function Row({
  subscription,
  onDelete,
  onToggle,
  onTest,
  onEdit,
}: {
  subscription: ReportSubscription;
  onDelete: (id: string) => void;
  onToggle: (id: string, isActive: boolean) => void;
  onTest: (id: string) => void;
  onEdit: (subscription: ReportSubscription) => void;
}) {
  return (
    <tr className="border-t border-line">
      <td className="px-3 py-2 text-xs text-ink">{subscription.report_id}</td>
      <td className="px-3 py-2 text-xs text-ink capitalize">{subscription.schedule}</td>
      <td className="px-3 py-2 text-xs text-ink">{subscription.timezone}</td>
      <td className="px-3 py-2 text-xs text-ink">
        {subscription.channel}: {subscription.target}
      </td>
      <td className="px-3 py-2 text-xs text-ink-2">{subscription.next_run_at}</td>
      <td className="px-3 py-2 text-xs">
        <span className={`px-1.5 py-0.5 rounded border ${subscription.is_active ? 'border-spark text-spark' : 'border-line text-ink-3'}`}>
          {subscription.is_active ? 'Active' : 'Inactive'}
        </span>
      </td>
      <td className="px-3 py-2">
        <div className="flex items-center gap-1">
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={() => onTest(subscription.id)}>
            <FlaskConical className="w-3 h-3 mr-1" />
            Test
          </Button>
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={() => onEdit(subscription)}>
            Edit
          </Button>
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={() => onToggle(subscription.id, !subscription.is_active)}>
            {subscription.is_active ? 'Pause' : 'Resume'}
          </Button>
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs text-down" onClick={() => onDelete(subscription.id)}>
            <Trash2 className="w-3 h-3" />
          </Button>
        </div>
      </td>
    </tr>
  );
}

export function SubscriptionsTable({ websiteId }: SubscriptionsTableProps) {
  const { data, isLoading } = useReportSubscriptions(websiteId);
  const subscriptions = data?.data ?? [];
  const deleteSubscription = useDeleteReportSubscription(websiteId);
  const updateSubscription = useUpdateReportSubscription(websiteId);
  const testSubscription = useTestReportSubscription(websiteId);

  function handleEdit(subscription: ReportSubscription) {
    const schedule = window.prompt('Schedule (daily|weekly|monthly)', subscription.schedule);
    if (schedule === null) return;
    const channel = window.prompt('Channel (email|webhook)', subscription.channel);
    if (channel === null) return;
    const target = window.prompt('Target', subscription.target);
    if (target === null) return;
    const timezone = window.prompt('Timezone', subscription.timezone);
    if (timezone === null) return;

    updateSubscription.mutate({
      subscriptionId: subscription.id,
      payload: {
        schedule: schedule as SubscriptionSchedule,
        channel: channel as NotificationChannel,
        target,
        timezone,
      },
    });
  }

  return (
    <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
      <table className="w-full text-left">
        <thead className="bg-surface-2">
          <tr className="text-xs text-ink-3">
            <th className="px-3 py-2 font-medium">Report</th>
            <th className="px-3 py-2 font-medium">Schedule</th>
            <th className="px-3 py-2 font-medium">Timezone</th>
            <th className="px-3 py-2 font-medium">Delivery</th>
            <th className="px-3 py-2 font-medium">Next run</th>
            <th className="px-3 py-2 font-medium">Status</th>
            <th className="px-3 py-2 font-medium">Actions</th>
          </tr>
        </thead>
        <tbody>
          {isLoading ? (
            <tr>
              <td colSpan={7} className="px-3 py-6 text-sm text-ink-3">Loading subscriptions…</td>
            </tr>
          ) : subscriptions.length === 0 ? (
            <tr>
              <td colSpan={7} className="px-3 py-6 text-sm text-ink-3">No subscriptions configured.</td>
            </tr>
          ) : (
            subscriptions.map((subscription) => (
              <Row
                key={subscription.id}
                subscription={subscription}
                onDelete={(id) => deleteSubscription.mutate(id)}
                onTest={(id) => testSubscription.mutate(id)}
                onToggle={(id, isActive) => updateSubscription.mutate({ subscriptionId: id, payload: { is_active: isActive } })}
                onEdit={handleEdit}
              />
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}
