'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { getRuntimeAuthMode } from '@/lib/runtime';

export const AUTH_QUERY_KEY = ['auth'] as const;

export function useAuth() {
  const runtimeAuthMode = getRuntimeAuthMode();
  const authDisabled = runtimeAuthMode === 'none';

  return useQuery({
    queryKey: AUTH_QUERY_KEY,
    queryFn: () => api.getAuthStatus(),
    enabled: !authDisabled,
    initialData: authDisabled ? null : undefined,
    staleTime: 5 * 60 * 1000,
    retry: false,
  });
}
