'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

export function useEvents(websiteId: string) {
  const { dateRange, filters } = useFilters();

  return useQuery({
    queryKey: ['events', websiteId, dateRange, filters],
    queryFn: () => api.getEventNames(websiteId, { ...dateRange, ...filters }),
    enabled: !!websiteId,
    staleTime: 60_000,
  });
}
