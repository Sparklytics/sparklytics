'use client';

import { JourneyBranch, JourneyDirection } from '@/lib/api';
import { JourneyBranchRow } from './JourneyBranchRow';

interface JourneyBranchListProps {
  branches: JourneyBranch[];
  direction: JourneyDirection;
  onPickNode: (value: string) => void;
}

export function JourneyBranchList({ branches, direction, onPickNode }: JourneyBranchListProps) {
  if (branches.length === 0) {
    return (
      <div className="border border-line rounded-lg bg-surface-1 px-4 py-10 text-center">
        <p className="text-sm font-medium text-ink">No branches in this period</p>
        <p className="text-xs text-ink-3 mt-1">
          Try another anchor or a wider date range.
        </p>
      </div>
    );
  }

  const maxSessions = branches.reduce((max, branch) => Math.max(max, branch.sessions), 0);

  return (
    <div className="space-y-2">
      {branches.map((branch, idx) => (
        <JourneyBranchRow
          key={`${idx}:${branch.nodes.join('|')}:${branch.sessions}`}
          branch={branch}
          direction={direction}
          maxSessions={maxSessions}
          onPickNode={onPickNode}
        />
      ))}
    </div>
  );
}
