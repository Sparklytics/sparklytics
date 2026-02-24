'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

export function useFunnelResults(websiteId: string, funnelId: string) {
  const { dateRange, filters } = useFilters();

  return useQuery({
    queryKey: ['funnel-results', websiteId, funnelId, dateRange, filters],
    queryFn: () => api.getFunnelResults(websiteId, funnelId, { ...dateRange, ...filters }),
    enabled: !!websiteId && !!funnelId,
    staleTime: 5 * 60_000,
  });
}
