'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  api,
  BotDateRangeParams,
  BotListParams,
  BotReportParams,
  BotRecomputePayload,
  CreateBotListEntryPayload,
  UpdateBotPolicyPayload,
} from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useBotSummary(websiteId: string, params: BotDateRangeParams) {
  return useQuery({
    queryKey: ['bot-summary', websiteId, params],
    queryFn: () => api.getBotSummary(websiteId, params),
    enabled: !!websiteId,
    staleTime: 30_000,
  });
}

export function useBotPolicy(websiteId: string) {
  return useQuery({
    queryKey: ['bot-policy', websiteId],
    queryFn: () => api.getBotPolicy(websiteId),
    enabled: !!websiteId,
    staleTime: 30_000,
  });
}

export function useUpdateBotPolicy(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (payload: UpdateBotPolicyPayload) => api.updateBotPolicy(websiteId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bot-policy', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['bot-summary', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['bot-report', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['bot-audit', websiteId] });
      toast({ title: 'Bot policy updated' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to update bot policy',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useBotAllowlist(websiteId: string, params: BotListParams = {}) {
  return useQuery({
    queryKey: ['bot-allowlist', websiteId, params],
    queryFn: () => api.listBotAllowlist(websiteId, params),
    enabled: !!websiteId,
    staleTime: 15_000,
  });
}

export function useCreateBotAllowlist(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (payload: CreateBotListEntryPayload) => api.createBotAllowlist(websiteId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bot-allowlist', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['bot-audit', websiteId] });
      toast({ title: 'Allowlist entry added' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to add allowlist entry',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useDeleteBotAllowlist(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (entryId: string) => api.deleteBotAllowlist(websiteId, entryId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bot-allowlist', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['bot-audit', websiteId] });
      toast({ title: 'Allowlist entry removed' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to remove allowlist entry',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useBotBlocklist(websiteId: string, params: BotListParams = {}) {
  return useQuery({
    queryKey: ['bot-blocklist', websiteId, params],
    queryFn: () => api.listBotBlocklist(websiteId, params),
    enabled: !!websiteId,
    staleTime: 15_000,
  });
}

export function useCreateBotBlocklist(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (payload: CreateBotListEntryPayload) => api.createBotBlocklist(websiteId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bot-blocklist', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['bot-audit', websiteId] });
      toast({ title: 'Blocklist entry added' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to add blocklist entry',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useDeleteBotBlocklist(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (entryId: string) => api.deleteBotBlocklist(websiteId, entryId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bot-blocklist', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['bot-audit', websiteId] });
      toast({ title: 'Blocklist entry removed' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to remove blocklist entry',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useBotReport(websiteId: string, params: BotReportParams) {
  return useQuery({
    queryKey: ['bot-report', websiteId, params],
    queryFn: () => api.getBotReport(websiteId, params),
    enabled: !!websiteId,
    staleTime: 30_000,
  });
}

export function useBotAudit(websiteId: string, params: BotListParams = {}) {
  return useQuery({
    queryKey: ['bot-audit', websiteId, params],
    queryFn: () => api.listBotAudit(websiteId, params),
    enabled: !!websiteId,
    staleTime: 15_000,
  });
}

export function useStartBotRecompute(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (payload: BotRecomputePayload) => api.startBotRecompute(websiteId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bot-audit', websiteId] });
      toast({ title: 'Recompute started' });
    },
    onError: (error: Error) => {
      toast({
        title: 'Failed to start recompute',
        description: error.message,
        variant: 'destructive',
      });
    },
  });
}

export function useBotRecomputeStatus(websiteId: string, jobId: string | null) {
  return useQuery({
    queryKey: ['bot-recompute', websiteId, jobId],
    queryFn: () => api.getBotRecompute(websiteId, jobId as string),
    enabled: !!websiteId && !!jobId,
    staleTime: 5_000,
    refetchInterval: (query) => {
      const status = query.state.data?.data?.status;
      if (!status || status === 'queued' || status === 'running') {
        return 2000;
      }
      return false;
    },
  });
}
