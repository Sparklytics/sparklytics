'use client';

import { FileText, Zap } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { SessionEventItem } from '@/lib/api';

interface SessionTimelineItemProps {
  event: SessionEventItem;
  isLast: boolean;
}

export function SessionTimelineItem({ event, isLast }: SessionTimelineItemProps) {
  const isPageview = event.event_type === 'pageview';

  // Parse display URL (path only from full URL)
  let displayUrl = event.url;
  try {
    const u = new URL(event.url);
    displayUrl = u.pathname + u.search;
  } catch {
    // keep full url if parsing fails
  }

  // Format time as HH:MM:SS — handle both "2026-02-20 10:05:22" and ISO "T" separator
  const time = event.created_at.replace('T', ' ').split(' ')[1]?.slice(0, 8) ?? event.created_at;

  return (
    <li className="flex gap-3 pl-4">
      {/* Timeline dot + line */}
      <div className="flex flex-col items-center shrink-0">
        <div
          className={cn(
            'w-7 h-7 rounded-full flex items-center justify-center shrink-0 border border-line',
            isPageview ? 'bg-surface-1' : 'bg-spark/10'
          )}
        >
          {isPageview ? (
            <FileText className="w-3.5 h-3.5 text-ink-3" />
          ) : (
            <Zap className="w-3.5 h-3.5 text-spark" />
          )}
        </div>
        {!isLast && <div className="w-px flex-1 min-h-[16px] bg-line mt-1" />}
      </div>

      {/* Content */}
      <div className="pb-4 flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="text-xs text-ink-3 font-mono tabular-nums shrink-0">{time}</span>
          {!isPageview && event.event_name && (
            <span className="text-xs bg-spark/10 text-spark px-1.5 py-0.5 rounded-sm font-medium truncate">
              {event.event_name}
            </span>
          )}
        </div>
        <p className="text-sm text-ink truncate">{displayUrl}</p>
        {event.event_data && (
          <p className="text-xs text-ink-3 font-mono mt-0.5 truncate">{event.event_data}</p>
        )}
      </div>
    </li>
  );
}
