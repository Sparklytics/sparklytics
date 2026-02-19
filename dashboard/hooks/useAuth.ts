'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useAuth() {
  return useQuery({
    queryKey: ['auth'],
    queryFn: () => api.getAuthStatus(),
    staleTime: 5 * 60 * 1000,
    retry: false,
  });
}
