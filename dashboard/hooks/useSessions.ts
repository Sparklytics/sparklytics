'use client';

import { useInfiniteQuery } from '@tanstack/react-query';
import { api, SessionsResponse } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';

export function useSessions(websiteId: string) {
  const { dateRange, filters } = useFilters();

  return useInfiniteQuery<SessionsResponse>({
    queryKey: ['sessions', websiteId, dateRange, filters],
    queryFn: ({ pageParam }) =>
      api.getSessions(websiteId, {
        ...dateRange,
        ...filters,
        cursor: (pageParam as string | null) ?? undefined,
      }),
    getNextPageParam: (last) =>
      last.pagination.has_more ? last.pagination.next_cursor : undefined,
    initialPageParam: null,
    enabled: !!websiteId,
    staleTime: 60_000,
  });
}
