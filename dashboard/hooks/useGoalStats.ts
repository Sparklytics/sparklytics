'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

export function useGoalStats(websiteId: string, goalId: string) {
  const { dateRange, filters } = useFilters();

  return useQuery({
    queryKey: ['goal-stats', websiteId, goalId, dateRange, filters],
    queryFn: () => api.getGoalStats(websiteId, goalId, { ...dateRange, ...filters }),
    enabled: !!websiteId && !!goalId,
    staleTime: 5 * 60_000,
  });
}
