'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useApiKeys() {
  return useQuery({
    queryKey: ['apiKeys'],
    queryFn: () => api.listApiKeys(),
    staleTime: 60_000,
  });
}

export function useCreateApiKey() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (name: string) => api.createApiKey(name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['apiKeys'] });
      toast({ title: 'API key created' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to create API key', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDeleteApiKey() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (id: string) => api.deleteApiKey(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['apiKeys'] });
      toast({ title: 'API key revoked' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to revoke API key', description: error.message, variant: 'destructive' });
    },
  });
}
