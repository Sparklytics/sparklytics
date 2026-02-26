'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { toCompareParams, useFilters } from './useFilters';

export function useMetrics(websiteId: string, type: string, enabled = true) {
  const { dateRange, filters, compare } = useFilters();
  const compareParams = toCompareParams(compare);
  return useQuery({
    queryKey: ['metrics', websiteId, type, dateRange, filters, compare],
    queryFn: () => api.getMetrics(websiteId, type, { ...dateRange, ...filters, ...compareParams }),
    staleTime: 60 * 1000,
    refetchInterval: 60 * 1000,
    enabled: !!websiteId && !!type && enabled,
  });
}
