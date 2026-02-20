'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

export function useEventTimeseries(websiteId: string, eventName: string | null) {
  const { dateRange, filters } = useFilters();

  return useQuery({
    queryKey: ['eventTimeseries', websiteId, eventName, dateRange, filters],
    queryFn: () =>
      api.getEventTimeseries(websiteId, eventName!, { ...dateRange, ...filters }),
    enabled: !!websiteId && !!eventName,
    staleTime: 60_000,
  });
}
