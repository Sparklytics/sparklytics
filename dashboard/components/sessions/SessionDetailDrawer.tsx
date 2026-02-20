'use client';

import { formatDuration } from '@/lib/utils';
import { useSessionDetail } from '@/hooks/useSessionDetail';
import { SessionTimelineItem } from './SessionTimelineItem';
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';

interface SessionDetailDrawerProps {
  websiteId: string;
  sessionId: string | null;
  onClose: () => void;
}

function MetaChip({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-xs text-ink-3 uppercase tracking-wider">{label}</span>
      <span className="text-sm text-ink font-medium">{value}</span>
    </div>
  );
}

function SkeletonTimeline() {
  return (
    <ul className="space-y-0 pl-4">
      {Array.from({ length: 5 }).map((_, i) => (
        <li key={i} className="flex gap-3 pb-4 animate-pulse">
          <div className="w-7 h-7 rounded-full bg-surface-2 shrink-0" />
          <div className="flex-1 space-y-2 pt-1">
            <div className="h-3 bg-surface-2 rounded w-20" />
            <div className="h-4 bg-surface-2 rounded w-48" />
          </div>
        </li>
      ))}
    </ul>
  );
}

export function SessionDetailDrawer({
  websiteId,
  sessionId,
  onClose,
}: SessionDetailDrawerProps) {
  const { data, isLoading } = useSessionDetail(websiteId, sessionId);
  const detail = data?.data;

  return (
    <Sheet open={!!sessionId} onOpenChange={(open) => { if (!open) onClose(); }}>
      <SheetContent
        side="right"
        className="w-full sm:max-w-[480px] bg-canvas border-l border-line p-0 flex flex-col overflow-hidden"
      >
        {/* Header */}
        <SheetHeader className="px-4 py-3 border-b border-line shrink-0">
          <SheetTitle className="text-sm font-medium text-ink text-left">
            Session Detail
          </SheetTitle>
          {detail && (
            <div className="grid grid-cols-2 gap-x-4 gap-y-3 mt-3 sm:grid-cols-4">
              <MetaChip label="Duration" value={formatDuration(detail.session.duration_seconds)} />
              <MetaChip label="Pages" value={String(detail.session.pageview_count)} />
              <MetaChip label="Country" value={detail.session.country ?? '—'} />
              <MetaChip label="Browser" value={detail.session.browser ?? '—'} />
            </div>
          )}
        </SheetHeader>

        {/* Timeline */}
        <div className="flex-1 overflow-y-auto py-4">
          {detail?.truncated && (
            <div className="mx-4 mb-4 px-3 py-2 text-xs text-ink-3 bg-surface-1 border border-line rounded-md">
              Showing first 2,000 events — this session was truncated.
            </div>
          )}

          {isLoading ? (
            <SkeletonTimeline />
          ) : detail ? (
            detail.events.length > 0 ? (
              <ul>
                {detail.events.map((event, i) => (
                  <SessionTimelineItem
                    key={event.id}
                    event={event}
                    isLast={i === detail.events.length - 1}
                  />
                ))}
              </ul>
            ) : (
              <div className="px-4 py-8 text-center text-sm text-ink-3">
                No events found for this session.
              </div>
            )
          ) : null}
        </div>
      </SheetContent>
    </Sheet>
  );
}
