'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, CreateGoalPayload, UpdateGoalPayload } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useGoals(websiteId: string) {
  return useQuery({
    queryKey: ['goals', websiteId],
    queryFn: () => api.listGoals(websiteId),
    enabled: !!websiteId,
    staleTime: 60_000,
  });
}

export function useCreateGoal(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: CreateGoalPayload) => api.createGoal(websiteId, payload),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['goals', websiteId] });
      toast({ title: 'Goal created', description: data.data.name });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to create goal', description: error.message, variant: 'destructive' });
    },
  });
}

export function useUpdateGoal(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: ({ goalId, payload }: { goalId: string; payload: UpdateGoalPayload }) =>
      api.updateGoal(websiteId, goalId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['goals', websiteId] });
      toast({ title: 'Goal updated' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to update goal', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDeleteGoal(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (goalId: string) => api.deleteGoal(websiteId, goalId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['goals', websiteId] });
      toast({ title: 'Goal deleted' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to delete goal', description: error.message, variant: 'destructive' });
    },
  });
}
