'use client';

import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api, JourneyParams } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

type JourneyControlParams = Pick<
  JourneyParams,
  'anchor_type' | 'anchor_value' | 'direction' | 'max_depth'
>;

export function useJourney(websiteId: string, params: JourneyControlParams) {
  const { dateRange, filters } = useFilters();

  const queryParams: JourneyParams = useMemo(
    () => ({
      anchor_type: params.anchor_type,
      anchor_value: params.anchor_value,
      direction: params.direction,
      max_depth: params.max_depth,
      start_date: dateRange.start_date,
      end_date: dateRange.end_date,
      ...(filters as Partial<JourneyParams>),
    }),
    [dateRange.end_date, dateRange.start_date, filters, params.anchor_type, params.anchor_value, params.direction, params.max_depth]
  );

  return useQuery({
    queryKey: ['journey', websiteId, queryParams],
    queryFn: () => api.getJourney(websiteId, queryParams),
    enabled: !!websiteId && queryParams.anchor_value.trim().length > 0,
    staleTime: 60_000,
    placeholderData: (previousData) => previousData,
  });
}
