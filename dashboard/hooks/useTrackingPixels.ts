'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, CreateTrackingPixelPayload, UpdateTrackingPixelPayload } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useTrackingPixels(websiteId: string) {
  return useQuery({
    queryKey: ['tracking-pixels', websiteId],
    queryFn: () => api.listTrackingPixels(websiteId),
    enabled: !!websiteId,
    staleTime: 30_000,
  });
}

export function useTrackingPixelStats(websiteId: string, pixelId: string | null) {
  return useQuery({
    queryKey: ['tracking-pixel-stats', websiteId, pixelId],
    queryFn: () => api.getTrackingPixelStats(websiteId, pixelId as string),
    enabled: !!websiteId && !!pixelId,
    staleTime: 15_000,
  });
}

export function useCreateTrackingPixel(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: CreateTrackingPixelPayload) => api.createTrackingPixel(websiteId, payload),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['tracking-pixels', websiteId] });
      toast({ title: 'Tracking pixel created', description: data.data.name });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to create tracking pixel',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useUpdateTrackingPixel(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: ({ pixelId, payload }: { pixelId: string; payload: UpdateTrackingPixelPayload }) =>
      api.updateTrackingPixel(websiteId, pixelId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tracking-pixels', websiteId] });
      toast({ title: 'Tracking pixel updated' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to update tracking pixel',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useDeleteTrackingPixel(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (pixelId: string) => api.deleteTrackingPixel(websiteId, pixelId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tracking-pixels', websiteId] });
      toast({ title: 'Tracking pixel deleted' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to delete tracking pixel',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}
