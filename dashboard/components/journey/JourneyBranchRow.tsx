'use client';

import { JourneyBranch, JourneyDirection } from '@/lib/api';

interface JourneyBranchRowProps {
  branch: JourneyBranch;
  direction: JourneyDirection;
  maxSessions: number;
  onPickNode: (value: string) => void;
}

export function JourneyBranchRow({
  branch,
  direction,
  maxSessions,
  onPickNode,
}: JourneyBranchRowProps) {
  const widthPercent = maxSessions > 0 ? Math.max((branch.sessions / maxSessions) * 100, 1) : 0;

  return (
    <div className="border border-line rounded-md bg-surface-1 p-3 space-y-2">
      <div className="flex items-start gap-3">
        <div className="flex-1 min-w-0 flex flex-wrap items-center gap-1">
          {branch.nodes.length === 0 ? (
            <span className="text-xs text-ink-3 italic">
              {direction === 'next' ? '(no next step)' : '(no previous step)'}
            </span>
          ) : (
            branch.nodes.map((node, idx) => (
              <span key={`${node}:${idx}`} className="inline-flex items-center gap-1">
                {idx > 0 && <span className="text-xs text-ink-4">→</span>}
                <button
                  onClick={() => onPickNode(node)}
                  className="px-2 py-1 rounded-sm border border-line text-xs font-mono text-ink-2 hover:text-ink hover:border-spark transition-colors"
                  type="button"
                >
                  {node}
                </button>
              </span>
            ))
          )}
        </div>

        <div className="text-right shrink-0">
          <div className="font-mono tabular-nums text-xs text-ink">
            {branch.sessions.toLocaleString()}
          </div>
          <div className="font-mono tabular-nums text-xs text-ink-3">
            {(branch.share * 100).toFixed(1)}%
          </div>
        </div>
      </div>

      <div className="h-1 rounded-sm bg-surface-2 overflow-hidden">
        <div
          className="h-full bg-spark"
          style={{ width: `${Math.min(widthPercent, 100)}%` }}
        />
      </div>
    </div>
  );
}
