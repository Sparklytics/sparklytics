'use client';

import { useState } from 'react';
import { useSessions } from '@/hooks/useSessions';
import { SessionsTable } from './SessionsTable';
import { SessionDetailDrawer } from './SessionDetailDrawer';

interface SessionsPageProps {
  websiteId: string;
}

export function SessionsPage({ websiteId }: SessionsPageProps) {
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);

  const {
    data,
    isFetching,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
  } = useSessions(websiteId);

  const sessions = data?.pages.flatMap((p) => p.data) ?? [];

  return (
    <div>
      <SessionsTable
        sessions={sessions}
        hasNextPage={!!hasNextPage}
        isFetchingNextPage={isFetchingNextPage}
        isFetching={isFetching}
        fetchNextPage={fetchNextPage}
        selectedSessionId={selectedSessionId}
        onSelect={setSelectedSessionId}
      />

      <SessionDetailDrawer
        websiteId={websiteId}
        sessionId={selectedSessionId}
        onClose={() => setSelectedSessionId(null)}
      />
    </div>
  );
}
