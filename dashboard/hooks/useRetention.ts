'use client';

import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api, RetentionParams } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

type RetentionControlParams = Pick<
  RetentionParams,
  'cohort_granularity' | 'max_periods'
>;

export function useRetention(websiteId: string, params: RetentionControlParams) {
  const { dateRange, filters } = useFilters();

  const queryParams: RetentionParams = useMemo(
    () => ({
      start_date: dateRange.start_date,
      end_date: dateRange.end_date,
      cohort_granularity: params.cohort_granularity,
      max_periods: params.max_periods,
      ...(filters as Partial<RetentionParams>),
    }),
    [
      dateRange.end_date,
      dateRange.start_date,
      filters,
      params.cohort_granularity,
      params.max_periods,
    ]
  );

  return useQuery({
    queryKey: ['retention', websiteId, queryParams],
    queryFn: () => api.getRetention(websiteId, queryParams),
    enabled: !!websiteId,
    staleTime: 120_000,
    placeholderData: (previousData) => previousData,
  });
}
