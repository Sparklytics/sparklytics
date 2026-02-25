'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from './useFilters';

export function useMetrics(websiteId: string, type: string, enabled = true) {
  const { dateRange, filters, compare } = useFilters();
  const compareParams =
    compare.mode === 'custom'
      ? {
          compare_mode: compare.mode,
          compare_start_date: compare.compare_start_date,
          compare_end_date: compare.compare_end_date,
        }
      : { compare_mode: compare.mode };
  return useQuery({
    queryKey: ['metrics', websiteId, type, dateRange, filters, compare],
    queryFn: () => api.getMetrics(websiteId, type, { ...dateRange, ...filters, ...compareParams }),
    staleTime: 60 * 1000,
    refetchInterval: 60 * 1000,
    enabled: !!websiteId && !!type && enabled,
  });
}
