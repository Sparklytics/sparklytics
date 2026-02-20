'use client';

import { useState, useEffect } from 'react';
import { useEvents } from '@/hooks/useEvents';
import { EventsTable } from './EventsTable';
import { EventDetailPanel } from './EventDetailPanel';

interface EventsPageProps {
  websiteId: string;
}

export function EventsPage({ websiteId }: EventsPageProps) {
  const [selectedEvent, setSelectedEvent] = useState<string | null>(null);
  const [isMobile, setIsMobile] = useState(false);
  const { data, isLoading } = useEvents(websiteId);

  useEffect(() => {
    function check() {
      setIsMobile(window.innerWidth < 768);
    }
    check();
    window.addEventListener('resize', check);
    return () => window.removeEventListener('resize', check);
  }, []);

  const rows = data?.data?.rows ?? [];
  const total = data?.data?.total ?? 0;
  const hasMore = total > rows.length;

  return (
    <div className="flex gap-6">
      {/* Left: event name list — full width when no panel, flex-1 when panel open */}
      <div className={selectedEvent && !isMobile ? 'flex-1 min-w-0' : 'w-full'}>
        <EventsTable
          rows={rows}
          total={total}
          hasMore={hasMore}
          loading={isLoading}
          selectedEvent={selectedEvent}
          onSelectEvent={setSelectedEvent}
        />
      </div>

      {/* Right: detail panel (visible when an event is selected) */}
      {selectedEvent && (
        <EventDetailPanel
          websiteId={websiteId}
          eventName={selectedEvent}
          onClose={() => setSelectedEvent(null)}
          isFullscreen={isMobile}
        />
      )}
    </div>
  );
}
