'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, CreateFunnelPayload, UpdateFunnelPayload } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useFunnels(websiteId: string) {
  return useQuery({
    queryKey: ['funnels', websiteId],
    queryFn: () => api.listFunnels(websiteId),
    enabled: !!websiteId,
    staleTime: 60_000,
  });
}

export function useCreateFunnel(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: CreateFunnelPayload) => api.createFunnel(websiteId, payload),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['funnels', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['funnel-results', websiteId] });
      toast({ title: 'Funnel created', description: data.data.name });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to create funnel', description: error.message, variant: 'destructive' });
    },
  });
}

export function useUpdateFunnel(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: ({ funnelId, payload }: { funnelId: string; payload: UpdateFunnelPayload }) =>
      api.updateFunnel(websiteId, funnelId, payload),
    onSuccess: (_data, { funnelId }) => {
      queryClient.invalidateQueries({ queryKey: ['funnels', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['funnel', websiteId, funnelId] });
      queryClient.invalidateQueries({ queryKey: ['funnel-results', websiteId] });
      toast({ title: 'Funnel updated' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to update funnel', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDeleteFunnel(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (funnelId: string) => api.deleteFunnel(websiteId, funnelId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['funnels', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['funnel-results', websiteId] });
      toast({ title: 'Funnel deleted' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to delete funnel', description: error.message, variant: 'destructive' });
    },
  });
}
