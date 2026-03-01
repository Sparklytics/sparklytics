'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function usePlanLimits(enabled = true) {
  return useQuery({
    queryKey: ['admin-plan-limits'],
    queryFn: () => api.listPlanLimits(),
    enabled,
    staleTime: 30 * 1000,
  });
}

export function useUpdatePlanLimit() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (input: { plan: string; peak_events_per_sec: number; monthly_event_limit: number }) =>
      api.updatePlanLimit(input.plan, {
        peak_events_per_sec: input.peak_events_per_sec,
        monthly_event_limit: input.monthly_event_limit,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-plan-limits'] });
      queryClient.invalidateQueries({ queryKey: ['admin-tenant-limits'] });
      queryClient.invalidateQueries({ queryKey: ['admin-tenant-usage'] });
      queryClient.invalidateQueries({ queryKey: ['usage'] });
      toast({ title: 'Plan limits updated' });
    },
    onError: (error: Error) => {
      toast({ title: 'Update failed', description: error.message, variant: 'destructive' });
    },
  });
}

export function useTenantLimits(tenantId: string, enabled = true) {
  return useQuery({
    queryKey: ['admin-tenant-limits', tenantId],
    queryFn: () => api.getTenantLimits(tenantId),
    enabled: enabled && !!tenantId,
    staleTime: 15 * 1000,
  });
}

export function useUpdateTenantLimits(tenantId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: {
      peak_events_per_sec?: number | null;
      monthly_event_limit?: number | null;
      clear?: boolean;
    }) => api.updateTenantLimits(tenantId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-tenant-limits', tenantId] });
      queryClient.invalidateQueries({ queryKey: ['admin-tenant-usage', tenantId] });
      toast({ title: 'Tenant limits updated' });
    },
    onError: (error: Error) => {
      toast({ title: 'Update failed', description: error.message, variant: 'destructive' });
    },
  });
}

export function useTenantUsage(tenantId: string, month?: string, enabled = true) {
  return useQuery({
    queryKey: ['admin-tenant-usage', tenantId, month ?? 'current'],
    queryFn: () => api.getTenantUsage(tenantId, month),
    enabled: enabled && !!tenantId,
    staleTime: 15 * 1000,
  });
}
