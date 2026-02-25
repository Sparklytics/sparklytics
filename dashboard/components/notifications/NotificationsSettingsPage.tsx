'use client';

import { AlertsTable } from './AlertsTable';
import { CreateAlertDialog } from './CreateAlertDialog';
import { CreateSubscriptionDialog } from './CreateSubscriptionDialog';
import { DeliveryHistoryTable } from './DeliveryHistoryTable';
import { SubscriptionsTable } from './SubscriptionsTable';

interface NotificationsSettingsPageProps {
  websiteId: string;
}

export function NotificationsSettingsPage({ websiteId }: NotificationsSettingsPageProps) {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink">Notifications</h2>
        <p className="text-xs text-ink-3 mt-0.5">
          Configure scheduled reports and anomaly alerts for this website.
        </p>
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
