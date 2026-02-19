'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useRealtime(websiteId: string) {
  return useQuery({
    queryKey: ['realtime', websiteId],
    queryFn: () => api.getRealtime(websiteId),
    refetchInterval: 30 * 1000,
    enabled: !!websiteId,
  });
}
