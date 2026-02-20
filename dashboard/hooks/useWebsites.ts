'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useWebsites() {
  return useQuery({
    queryKey: ['websites'],
    queryFn: () => api.getWebsites(),
    staleTime: 5 * 60 * 1000,
  });
}

export function useWebsite(websiteId: string) {
  return useQuery({
    queryKey: ['website', websiteId],
    queryFn: () => api.getWebsite(websiteId),
    enabled: !!websiteId,
    staleTime: 5 * 60 * 1000,
  });
}

export function useCreateWebsite() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: { name: string; domain: string; timezone: string }) =>
      api.createWebsite(payload),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['websites'] });
      toast({ title: 'Website created', description: data.data.name });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to create website', description: error.message, variant: 'destructive' });
    },
  });
}

export function useUpdateWebsite(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: { name?: string; domain?: string; timezone?: string }) =>
      api.updateWebsite(websiteId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['websites'] });
      queryClient.invalidateQueries({ queryKey: ['website', websiteId] });
      toast({ title: 'Settings saved' });
    },
    onError: (error: Error) => {
      toast({ title: 'Save failed', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDeleteWebsite() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (id: string) => api.deleteWebsite(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['websites'] });
      toast({ title: 'Website deleted' });
    },
    onError: (error: Error) => {
      toast({ title: 'Delete failed', description: error.message, variant: 'destructive' });
    },
  });
}
