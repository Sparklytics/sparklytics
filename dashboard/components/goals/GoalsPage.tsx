'use client';

import { useState } from 'react';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { GoalsList } from './GoalsList';
import { GoalFormDialog } from './GoalFormDialog';

interface GoalsPageProps {
  websiteId: string;
}

export function GoalsPage({ websiteId }: GoalsPageProps) {
  const [createDialogOpen, setCreateDialogOpen] = useState(false);

  return (
    <div className="space-y-4">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-ink">Goals</h2>
          <p className="text-xs text-ink-3 mt-0.5">
            Track conversions by page views or custom events.
          </p>
        </div>
        <Button
          size="sm"
          onClick={() => setCreateDialogOpen(true)}
          className="text-xs gap-1"
        >
          <Plus className="w-3.5 h-3.5" />
          New Goal
        </Button>
      </div>

      <GoalsList websiteId={websiteId} />

      <GoalFormDialog
        websiteId={websiteId}
        open={createDialogOpen}
        onClose={() => setCreateDialogOpen(false)}
      />
    </div>
  );
}
