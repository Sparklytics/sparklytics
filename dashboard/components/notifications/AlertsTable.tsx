'use client';

import { useState } from 'react';
import { FlaskConical, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useAlertRules, useDeleteAlertRule, useTestAlertRule, useUpdateAlertRule } from '@/hooks/useNotifications';
import { EditAlertDialog } from './EditAlertDialog';
import type { AlertRule } from '@/lib/api';

interface AlertsTableProps {
  websiteId: string;
}

function Row({
  rule,
  onDelete,
  onToggle,
  onTest,
  onEdit,
}: {
  rule: AlertRule;
  onDelete: (id: string) => void;
  onToggle: (id: string, isActive: boolean) => void;
  onTest: (id: string) => void;
  onEdit: (rule: AlertRule) => void;
}) {
  return (
    <tr className="border-t border-line">
      <td className="px-3 py-2 text-sm text-ink">{rule.name}</td>
      <td className="px-3 py-2 text-xs text-ink">{rule.metric}</td>
      <td className="px-3 py-2 text-xs text-ink">{rule.condition_type}</td>
      <td className="px-3 py-2 text-xs text-ink">{rule.threshold_value}</td>
      <td className="px-3 py-2 text-xs text-ink">
        {rule.channel}: {rule.target}
      </td>
      <td className="px-3 py-2 text-xs">
        <span className={`px-1.5 py-0.5 rounded-sm border ${rule.is_active ? 'border-spark text-spark' : 'border-line text-ink-3'}`}>
          {rule.is_active ? 'Active' : 'Inactive'}
        </span>
      </td>
      <td className="px-3 py-2">
        <div className="flex items-center gap-1">
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={() => onTest(rule.id)}>
            <FlaskConical className="w-3 h-3 mr-1" />
            Test
          </Button>
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={() => onEdit(rule)}>
            Edit
          </Button>
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={() => onToggle(rule.id, !rule.is_active)}>
            {rule.is_active ? 'Pause' : 'Resume'}
          </Button>
          <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs text-down" onClick={() => onDelete(rule.id)}>
            <Trash2 className="w-3 h-3" />
          </Button>
        </div>
      </td>
    </tr>
  );
}

export function AlertsTable({ websiteId }: AlertsTableProps) {
  const { data, isLoading } = useAlertRules(websiteId);
  const updateAlert = useUpdateAlertRule(websiteId);
  const deleteAlert = useDeleteAlertRule(websiteId);
  const testAlert = useTestAlertRule(websiteId);
  const alerts = data?.data ?? [];
  const [editingRule, setEditingRule] = useState<AlertRule | null>(null);

  return (
    <>
      <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
        <table className="w-full text-left">
          <thead className="bg-surface-2">
            <tr className="text-xs text-ink-3">
              <th className="px-3 py-2 font-medium">Rule</th>
              <th className="px-3 py-2 font-medium">Metric</th>
              <th className="px-3 py-2 font-medium">Condition</th>
              <th className="px-3 py-2 font-medium">Threshold</th>
              <th className="px-3 py-2 font-medium">Delivery</th>
              <th className="px-3 py-2 font-medium">Status</th>
              <th className="px-3 py-2 font-medium">Actions</th>
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              <tr>
                <td colSpan={7} className="px-3 py-6 text-sm text-ink-3">Loading alerts...</td>
              </tr>
            ) : alerts.length === 0 ? (
              <tr>
                <td colSpan={7} className="px-3 py-6 text-sm text-ink-3">No alert rules configured.</td>
              </tr>
            ) : (
              alerts.map((rule) => (
                <Row
                  key={rule.id}
                  rule={rule}
                  onDelete={(id) => deleteAlert.mutate(id)}
                  onToggle={(id, isActive) => updateAlert.mutate({ alertId: id, payload: { is_active: isActive } })}
                  onTest={(id) => testAlert.mutate(id)}
                  onEdit={setEditingRule}
                />
              ))
            )}
          </tbody>
        </table>
      </div>

      <EditAlertDialog
        rule={editingRule}
        isPending={updateAlert.isPending}
        onSave={(alertId, payload) => {
          updateAlert.mutate({ alertId, payload }, { onSuccess: () => setEditingRule(null) });
        }}
        onClose={() => setEditingRule(null)}
      />
    </>
  );
}
