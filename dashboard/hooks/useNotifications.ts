'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  api,
  CreateAlertRulePayload,
  CreateReportSubscriptionPayload,
  UpdateAlertRulePayload,
  UpdateReportSubscriptionPayload,
} from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useReportSubscriptions(websiteId: string) {
  return useQuery({
    queryKey: ['report-subscriptions', websiteId],
    queryFn: () => api.listReportSubscriptions(websiteId),
    enabled: !!websiteId,
    staleTime: 30_000,
  });
}

export function useCreateReportSubscription(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (payload: CreateReportSubscriptionPayload) =>
      api.createReportSubscription(websiteId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['report-subscriptions', websiteId] });
      queryClient.invalidateQueries({ queryKey: ['notification-history', websiteId] });
      toast({ title: 'Subscription created' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to create subscription', description: error.message, variant: 'destructive' });
    },
  });
}

export function useUpdateReportSubscription(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({
      subscriptionId,
      payload,
    }: {
      subscriptionId: string;
      payload: UpdateReportSubscriptionPayload;
    }) => api.updateReportSubscription(websiteId, subscriptionId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['report-subscriptions', websiteId] });
      toast({ title: 'Subscription updated' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to update subscription', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDeleteReportSubscription(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (subscriptionId: string) => api.deleteReportSubscription(websiteId, subscriptionId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['report-subscriptions', websiteId] });
      toast({ title: 'Subscription deleted' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to delete subscription', description: error.message, variant: 'destructive' });
    },
  });
}

export function useTestReportSubscription(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (subscriptionId: string) => api.testReportSubscription(websiteId, subscriptionId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notification-history', websiteId] });
      toast({ title: 'Test delivery queued' });
    },
    onError: (error: Error) => {
      toast({ title: 'Test delivery failed', description: error.message, variant: 'destructive' });
    },
  });
}

export function useAlertRules(websiteId: string) {
  return useQuery({
    queryKey: ['alert-rules', websiteId],
    queryFn: () => api.listAlertRules(websiteId),
    enabled: !!websiteId,
    staleTime: 30_000,
  });
}

export function useCreateAlertRule(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (payload: CreateAlertRulePayload) => api.createAlertRule(websiteId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['alert-rules', websiteId] });
      toast({ title: 'Alert rule created' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to create alert rule', description: error.message, variant: 'destructive' });
    },
  });
}

export function useUpdateAlertRule(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ alertId, payload }: { alertId: string; payload: UpdateAlertRulePayload }) =>
      api.updateAlertRule(websiteId, alertId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['alert-rules', websiteId] });
      toast({ title: 'Alert rule updated' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to update alert rule', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDeleteAlertRule(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (alertId: string) => api.deleteAlertRule(websiteId, alertId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['alert-rules', websiteId] });
      toast({ title: 'Alert rule deleted' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to delete alert rule', description: error.message, variant: 'destructive' });
    },
  });
}

export function useTestAlertRule(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (alertId: string) => api.testAlertRule(websiteId, alertId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notification-history', websiteId] });
      toast({ title: 'Alert test sent' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to test alert', description: error.message, variant: 'destructive' });
    },
  });
}

export function useNotificationHistory(websiteId: string, limit = 50) {
  return useQuery({
    queryKey: ['notification-history', websiteId, limit],
    queryFn: () => api.getNotificationHistory(websiteId, limit),
    enabled: !!websiteId,
    staleTime: 10_000,
  });
}
