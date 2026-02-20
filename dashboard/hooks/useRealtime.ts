'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useRealtime(
  websiteId: string,
  refreshInterval = 30_000,
  enabled = true
) {
  return useQuery({
    queryKey: ['realtime', websiteId],
    queryFn: () => api.getRealtime(websiteId),
    refetchInterval: refreshInterval,
    staleTime: 0,
    enabled: !!websiteId && enabled,
  });
}
