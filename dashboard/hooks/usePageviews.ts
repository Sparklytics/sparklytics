'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from './useFilters';

export function usePageviews(websiteId: string, enabled = true) {
  const { dateRange, filters } = useFilters();
  return useQuery({
    queryKey: ['pageviews', websiteId, dateRange, filters],
    queryFn: () => api.getPageviews(websiteId, { ...dateRange, ...filters }),
    staleTime: 60 * 1000,
    refetchInterval: 60 * 1000,
    enabled: !!websiteId && enabled,
  });
}
