'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

export function useEventProperties(websiteId: string, eventName: string | null) {
  const { dateRange, filters } = useFilters();

  return useQuery({
    queryKey: ['eventProperties', websiteId, eventName, dateRange, filters],
    queryFn: () =>
      api.getEventProperties(websiteId, eventName!, { ...dateRange, ...filters }),
    enabled: !!websiteId && !!eventName,
    staleTime: 60_000,
  });
}
