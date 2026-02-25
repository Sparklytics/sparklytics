'use client';

import { cn } from '@/lib/utils';
import type { ReportType } from '@/lib/api';

interface ReportTypeBadgeProps {
  type: ReportType;
}

export function ReportTypeBadge({ type }: ReportTypeBadgeProps) {
  const label =
    type === 'stats'
      ? 'Stats'
      : type === 'pageviews'
        ? 'Pageviews'
        : type === 'metrics'
          ? 'Metrics'
          : 'Events';

  return (
    <span
      className={cn(
        'text-xs rounded-sm px-1.5 py-0.5 font-medium',
        type === 'stats' && 'bg-blue-500/10 text-blue-400',
        type === 'pageviews' && 'bg-indigo-500/10 text-indigo-400',
        type === 'metrics' && 'bg-amber-500/10 text-amber-400',
        type === 'events' && 'bg-fuchsia-500/10 text-fuchsia-400'
      )}
    >
      {label}
    </span>
  );
}
