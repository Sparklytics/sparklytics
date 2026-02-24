'use client';

import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { useDeleteFunnel } from '@/hooks/useFunnels';
import type { FunnelSummary } from '@/lib/api';

interface FunnelDeleteConfirmProps {
  websiteId: string;
  funnel: FunnelSummary | null;
  onClose: () => void;
}

export function FunnelDeleteConfirm({ websiteId, funnel, onClose }: FunnelDeleteConfirmProps) {
  const deleteFunnel = useDeleteFunnel(websiteId);

  function handleConfirm() {
    if (!funnel) return;
    deleteFunnel.mutate(funnel.id, {
      onSuccess: () => onClose(),
    });
  }

  return (
    <ConfirmDialog
      open={!!funnel}
      onOpenChange={(open) => { if (!open) onClose(); }}
      title="Delete funnel"
      description={`Are you sure you want to delete "${funnel?.name}"? This removes all steps and cannot be undone.`}
      confirmLabel="Delete"
      onConfirm={handleConfirm}
      destructive
      loading={deleteFunnel.isPending}
    />
  );
}
