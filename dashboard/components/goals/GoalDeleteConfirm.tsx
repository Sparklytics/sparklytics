'use client';

import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import { useDeleteGoal } from '@/hooks/useGoals';
import type { Goal } from '@/lib/api';

interface GoalDeleteConfirmProps {
  websiteId: string;
  goal: Goal | null;
  onClose: () => void;
}

export function GoalDeleteConfirm({ websiteId, goal, onClose }: GoalDeleteConfirmProps) {
  const deleteGoal = useDeleteGoal(websiteId);

  function handleConfirm() {
    if (!goal) return;
    deleteGoal.mutate(goal.id, {
      onSuccess: () => onClose(),
    });
  }

  return (
    <ConfirmDialog
      open={!!goal}
      onOpenChange={(open) => { if (!open) onClose(); }}
      title="Delete goal"
      description={`Are you sure you want to delete "${goal?.name}"? This action cannot be undone.`}
      confirmLabel="Delete"
      onConfirm={handleConfirm}
      destructive
      loading={deleteGoal.isPending}
    />
  );
}
