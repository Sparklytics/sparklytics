'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useUsage() {
  return useQuery({
    queryKey: ['usage'],
    queryFn: () => api.getUsage(),
    staleTime: 60 * 1000, // 1 minute
  });
}
