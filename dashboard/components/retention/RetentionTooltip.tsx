'use client';

import { RetentionGranularity } from '@/lib/api';
import { periodLabel } from '@/lib/retention-utils';

interface RetentionTooltipProps {
  id: string;
  cohortStart: string;
  granularity: RetentionGranularity;
  offset: number;
  retained: number;
  cohortSize: number;
  rate: number;
  notElapsed: boolean;
}

export function RetentionTooltip({
  id,
  cohortStart,
  granularity,
  offset,
  retained,
  cohortSize,
  rate,
  notElapsed,
}: RetentionTooltipProps) {
  return (
    <div
      id={id}
      role="tooltip"
      className="pointer-events-none hidden group-hover:block group-focus-within:block absolute z-20 left-1/2 top-full mt-1 -translate-x-1/2 min-w-[168px] rounded-md border border-line-3 bg-surface-2 px-2 py-2"
    >
      <p className="text-[10px] text-ink-3">{cohortStart}</p>
      <p className="text-[10px] text-ink-2 mt-1">{periodLabel(granularity, offset)}</p>
      {notElapsed ? (
        <p className="text-[10px] text-ink-4 mt-1">Not yet elapsed</p>
      ) : (
        <>
          <p className="text-[10px] font-mono tabular-nums text-ink mt-1">
            {retained.toLocaleString()} / {cohortSize.toLocaleString()}
          </p>
          <p className="text-[10px] font-mono tabular-nums text-ink-3">
            {(rate * 100).toFixed(1)}%
          </p>
        </>
      )}
    </div>
  );
}
