'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from './useFilters';

export function useStats(websiteId: string) {
  const { dateRange, filters } = useFilters();
  return useQuery({
    queryKey: ['stats', websiteId, dateRange, filters],
    queryFn: () => api.getStats(websiteId, { ...dateRange, ...filters }),
    staleTime: 60 * 1000,
    refetchInterval: 60 * 1000,
    enabled: !!websiteId,
  });
}
