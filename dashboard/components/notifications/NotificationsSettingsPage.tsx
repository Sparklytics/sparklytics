'use client';

import { useAlertRules, useReportSubscriptions, useNotificationHistory } from '@/hooks/useNotifications';
import { AlertsTable } from './AlertsTable';
import { CreateAlertDialog } from './CreateAlertDialog';
import { CreateSubscriptionDialog } from './CreateSubscriptionDialog';
import { DeliveryHistoryTable } from './DeliveryHistoryTable';
import { SubscriptionsTable } from './SubscriptionsTable';

interface NotificationsSettingsPageProps {
  websiteId: string;
}

export function NotificationsSettingsPage({ websiteId }: NotificationsSettingsPageProps) {
  const { data: subscriptions, isLoading: subsLoading } = useReportSubscriptions(websiteId);
  const { data: alertRules, isLoading: alertsLoading } = useAlertRules(websiteId);
  const { data: history, isLoading: historyLoading } = useNotificationHistory(websiteId);

  const activeSubscriptions = subscriptions?.data?.filter((s) => s.is_active).length ?? 0;
  const activeAlerts = alertRules?.data?.filter((a) => a.is_active).length ?? 0;
  const deliveryCount = history?.data?.length ?? 0;

  const summaryCards = [
    {
      label: 'Subscriptions',
      value: activeSubscriptions,
      loading: subsLoading,
    },
    {
      label: 'Alerts',
      value: activeAlerts,
      loading: alertsLoading,
    },
    {
      label: 'Deliveries',
      value: deliveryCount,
      loading: historyLoading,
    },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink">Notifications</h2>
        <p className="text-xs text-ink-3 mt-0.5">
          Configure scheduled reports and anomaly alerts for this website.
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        {summaryCards.map((card) => (
          <div
            key={card.label}
            className="border border-line rounded-lg bg-surface-1 p-4"
          >
            <p className="text-[11px] text-ink-3 uppercase tracking-[0.07em] font-medium truncate">
              {card.label}
            </p>
            {card.loading ? (
              <div className="mt-1 h-8 w-12 animate-pulse bg-surface-2 rounded" />
            ) : (
              <p className="text-2xl font-mono font-semibold tabular-nums text-ink mt-1">
                {card.value}
              </p>
            )}
          </div>
        ))}
      </div>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-ink">Scheduled reports</h3>
        <CreateSubscriptionDialog websiteId={websiteId} />
        <SubscriptionsTable websiteId={websiteId} />
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-ink">Alert rules</h3>
        <CreateAlertDialog websiteId={websiteId} />
        <AlertsTable websiteId={websiteId} />
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold text-ink">Delivery history</h3>
        <DeliveryHistoryTable websiteId={websiteId} />
      </section>
    </div>
  );
}
