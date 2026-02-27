'use client';

import { useNotificationHistory } from '@/hooks/useNotifications';

interface DeliveryHistoryTableProps {
  websiteId: string;
}

export function DeliveryHistoryTable({ websiteId }: DeliveryHistoryTableProps) {
  const { data, isLoading } = useNotificationHistory(websiteId, 50);
  const history = data?.data ?? [];

  return (
    <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
      <table className="w-full text-left">
        <thead className="bg-surface-2">
          <tr className="text-xs text-ink-3">
            <th className="px-3 py-2 font-medium">Time</th>
            <th className="px-3 py-2 font-medium">Source</th>
            <th className="px-3 py-2 font-medium">Status</th>
            <th className="px-3 py-2 font-medium">Error</th>
          </tr>
        </thead>
        <tbody>
          {isLoading ? (
            <tr>
              <td colSpan={4} className="px-3 py-6 text-sm text-ink-3">Loading delivery history…</td>
            </tr>
          ) : history.length === 0 ? (
            <tr>
              <td colSpan={4} className="px-3 py-6 text-sm text-ink-3">No deliveries recorded yet.</td>
            </tr>
          ) : (
            history.map((row) => (
              <tr key={row.id} className="border-t border-line">
                <td className="px-3 py-2 text-xs text-ink">{row.delivered_at}</td>
                <td className="px-3 py-2 text-xs text-ink">{row.source_type}:{row.source_id}</td>
                <td className="px-3 py-2 text-xs">
                  <span className={`px-1.5 py-0.5 rounded-sm border ${row.status === 'sent' ? 'border-spark text-spark' : 'border-down text-down'}`}>
                    {row.status}
                  </span>
                </td>
                <td className="px-3 py-2 text-xs text-ink-3">{row.error_message ?? '—'}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}
