'use client';

import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { IS_CLOUD } from '@/lib/config';
import { getRuntimeAuthMode } from '@/lib/runtime';

export function useUsage() {
  const runtimeAuthMode = getRuntimeAuthMode();

  return useQuery({
    queryKey: ['usage'],
    queryFn: () => api.getUsage(),
    enabled: IS_CLOUD && runtimeAuthMode !== 'none',
    staleTime: 60 * 1000, // 1 minute
  });
}
