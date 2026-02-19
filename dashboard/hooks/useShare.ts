'use client';

import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';

export function useEnableSharing(websiteId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.enableSharing(websiteId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['websites'] }),
  });
}

export function useDisableSharing(websiteId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.disableSharing(websiteId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['websites'] }),
  });
}
