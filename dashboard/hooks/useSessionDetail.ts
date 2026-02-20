'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useSessionDetail(websiteId: string, sessionId: string | null) {
  return useQuery({
    queryKey: ['session-detail', websiteId, sessionId],
    queryFn: () => api.getSessionDetail(websiteId, sessionId!),
    enabled: !!websiteId && !!sessionId,
    staleTime: 5 * 60_000,
  });
}
