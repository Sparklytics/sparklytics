'use client';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  api,
  CreateReportPayload,
  UpdateReportPayload,
} from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function useReports(websiteId: string) {
  return useQuery({
    queryKey: ['reports', websiteId],
    queryFn: () => api.listReports(websiteId),
    enabled: !!websiteId,
    staleTime: 60_000,
  });
}

export function useReport(websiteId: string, reportId: string | null) {
  return useQuery({
    queryKey: ['report', websiteId, reportId],
    queryFn: () => api.getReport(websiteId, reportId as string),
    enabled: !!websiteId && !!reportId,
  });
}

export function useCreateReport(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (payload: CreateReportPayload) => api.createReport(websiteId, payload),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['reports', websiteId] });
      toast({ title: 'Report created', description: data.data.name });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to create report', description: error.message, variant: 'destructive' });
    },
  });
}

export function useUpdateReport(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: ({ reportId, payload }: { reportId: string; payload: UpdateReportPayload }) =>
      api.updateReport(websiteId, reportId, payload),
    onSuccess: (data, variables) => {
      queryClient.invalidateQueries({ queryKey: ['reports', websiteId] });
      queryClient.invalidateQueries({
        queryKey: ['report', websiteId, variables.reportId],
      });
      queryClient.setQueryData(['report', websiteId, variables.reportId], data);
      toast({ title: 'Report updated' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to update report', description: error.message, variant: 'destructive' });
    },
  });
}

export function useDeleteReport(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (reportId: string) => api.deleteReport(websiteId, reportId),
    onSuccess: (_, reportId) => {
      queryClient.invalidateQueries({ queryKey: ['reports', websiteId] });
      queryClient.removeQueries({ queryKey: ['report', websiteId, reportId] });
      toast({ title: 'Report deleted' });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to delete report', description: error.message, variant: 'destructive' });
    },
  });
}
