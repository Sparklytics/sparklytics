'use client';

import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useEnableSharing(websiteId: string) {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: () => api.enableSharing(websiteId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['websites'] });
      qc.invalidateQueries({ queryKey: ['website', websiteId] });
      toast({ title: 'Share link enabled' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to enable sharing', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDisableSharing(websiteId: string) {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: () => api.disableSharing(websiteId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['websites'] });
      qc.invalidateQueries({ queryKey: ['website', websiteId] });
      toast({ title: 'Share link disabled' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to disable sharing', description: error.message, variant: 'destructive' });
    },
  });
}
