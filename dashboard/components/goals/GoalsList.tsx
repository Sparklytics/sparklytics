'use client';

import { useState } from 'react';
import { Pencil, Trash2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useGoals } from '@/hooks/useGoals';
import { useGoalStats } from '@/hooks/useGoalStats';
import { GoalStatsCard } from './GoalStatsCard';
import { GoalDeleteConfirm } from './GoalDeleteConfirm';
import { GoalFormDialog } from './GoalFormDialog';
import type { Goal } from '@/lib/api';

interface GoalsListProps {
  websiteId: string;
}

function GoalTypeBadge({ type }: { type: 'page_view' | 'event' }) {
  return (
    <span
      className={cn(
        'text-xs rounded-sm px-1.5 py-0.5 font-medium',
        type === 'page_view'
          ? 'bg-blue-500/10 text-blue-400'
          : 'bg-purple-500/10 text-purple-400'
      )}
    >
      {type === 'page_view' ? 'Page View' : 'Event'}
    </span>
  );
}

function GoalRow({
  goal,
  websiteId,
  onEdit,
  onDelete,
}: {
  goal: Goal;
  websiteId: string;
  onEdit: (goal: Goal) => void;
  onDelete: (goal: Goal) => void;
}) {
  const { data, isLoading } = useGoalStats(websiteId, goal.id);

  return (
    <tr className="border-b border-line hover:bg-surface-2/30 transition-colors">
      <td className="px-4 py-3 text-sm font-medium text-ink">{goal.name}</td>
      <td className="px-4 py-3">
        <GoalTypeBadge type={goal.goal_type} />
      </td>
      <td className="px-4 py-3 text-sm text-ink-2 max-w-[200px] truncate font-mono">
        {goal.match_value}
      </td>
      <td className="px-4 py-3">
        <GoalStatsCard stats={data?.data} loading={isLoading} variant="compact" />
      </td>
      <td className="px-4 py-3 font-mono tabular-nums text-sm text-ink-2">
        {data?.data?.conversions.toLocaleString() ?? '—'}
      </td>
      <td className="px-4 py-3">
        <div className="flex items-center gap-1 justify-end">
          <button
            onClick={() => onEdit(goal)}
            className="p-1.5 text-ink-3 hover:text-ink hover:bg-surface-2 rounded-md transition-colors"
            title="Edit goal"
          >
            <Pencil className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => onDelete(goal)}
            className="p-1.5 text-ink-3 hover:text-red-400 hover:bg-red-400/10 rounded-md transition-colors"
            title="Delete goal"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </td>
    </tr>
  );
}

export function GoalsList({ websiteId }: GoalsListProps) {
  const { data, isLoading } = useGoals(websiteId);
  const goals = data?.data ?? [];

  const [editingGoal, setEditingGoal] = useState<Goal | null>(null);
  const [deletingGoal, setDeletingGoal] = useState<Goal | null>(null);

  if (isLoading) {
    return (
      <div className="border border-line rounded-lg bg-surface-1 divide-y divide-line">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="px-4 py-4 flex items-center gap-4 animate-pulse">
            <div className="h-4 bg-surface-2 rounded flex-1" />
            <div className="h-4 bg-surface-2 rounded w-16" />
            <div className="h-4 bg-surface-2 rounded w-32" />
            <div className="h-4 bg-surface-2 rounded w-20" />
          </div>
        ))}
      </div>
    );
  }

  if (goals.length === 0) {
    return (
      <div className="border border-line rounded-lg bg-surface-1 px-6 py-16 text-center">
        <p className="text-sm font-medium text-ink mb-1">No goals yet</p>
        <p className="text-sm text-ink-3">Click New Goal to define your first conversion.</p>
      </div>
    );
  }

  return (
    <>
      <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-xs font-medium text-ink-3 uppercase tracking-wider">
                <th className="px-4 py-2 text-left">Goal Name</th>
                <th className="px-4 py-2 text-left">Type</th>
                <th className="px-4 py-2 text-left">Match Value</th>
                <th className="px-4 py-2 text-left">Conv. Rate</th>
                <th className="px-4 py-2 text-left">Events</th>
                <th className="px-4 py-2 text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {goals.map((goal) => (
                <GoalRow
                  key={goal.id}
                  goal={goal}
                  websiteId={websiteId}
                  onEdit={setEditingGoal}
                  onDelete={setDeletingGoal}
                />
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <GoalFormDialog
        websiteId={websiteId}
        open={!!editingGoal}
        onClose={() => setEditingGoal(null)}
        editingGoal={editingGoal}
      />

      <GoalDeleteConfirm
        websiteId={websiteId}
        goal={deletingGoal}
        onClose={() => setDeletingGoal(null)}
      />
    </>
  );
}
