'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, CreateCampaignLinkPayload, UpdateCampaignLinkPayload } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useCampaignLinks(websiteId: string) {
  return useQuery({
    queryKey: ['campaign-links', websiteId],
    queryFn: () => api.listCampaignLinks(websiteId),
    enabled: !!websiteId,
    staleTime: 30_000,
  });
}

export function useCampaignLinkStats(websiteId: string, linkId: string | null) {
  return useQuery({
    queryKey: ['campaign-link-stats', websiteId, linkId],
    queryFn: () => api.getCampaignLinkStats(websiteId, linkId as string),
    enabled: !!websiteId && !!linkId,
    staleTime: 15_000,
  });
}

export function useCreateCampaignLink(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: CreateCampaignLinkPayload) => api.createCampaignLink(websiteId, payload),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['campaign-links', websiteId] });
      toast({ title: 'Campaign link created', description: data.data.name });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to create campaign link',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useUpdateCampaignLink(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: ({ linkId, payload }: { linkId: string; payload: UpdateCampaignLinkPayload }) =>
      api.updateCampaignLink(websiteId, linkId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['campaign-links', websiteId] });
      toast({ title: 'Campaign link updated' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to update campaign link',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useDeleteCampaignLink(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (linkId: string) => api.deleteCampaignLink(websiteId, linkId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['campaign-links', websiteId] });
      toast({ title: 'Campaign link deleted' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to delete campaign link',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}
