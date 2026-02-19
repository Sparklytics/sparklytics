'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useWebsites() {
  return useQuery({
    queryKey: ['websites'],
    queryFn: () => api.getWebsites(),
    staleTime: 5 * 60 * 1000,
  });
}
