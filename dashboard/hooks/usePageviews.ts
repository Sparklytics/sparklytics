'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { toCompareParams, useFilters } from './useFilters';

export function usePageviews(websiteId: string, enabled = true) {
  const { dateRange, filters, compare } = useFilters();
  const compareParams = toCompareParams(compare);
  return useQuery({
    queryKey: ['pageviews', websiteId, dateRange, filters, compare],
    queryFn: () => api.getPageviews(websiteId, { ...dateRange, ...filters, ...compareParams }),
    staleTime: 60 * 1000,
    refetchInterval: 60 * 1000,
    enabled: !!websiteId && enabled,
  });
}
