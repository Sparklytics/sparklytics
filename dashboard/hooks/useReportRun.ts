'use client';

import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api, ReportConfig } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function usePreviewReport(websiteId: string) {
  const { toast } = useToast();

  return useMutation({
    mutationFn: (config: ReportConfig) => api.previewReport(websiteId, config),
    onError: (error: Error) => {
      toast({ title: 'Failed to preview report', description: error.message, variant: 'destructive' });
    },
  });
}

export function useRunReport(websiteId: string) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: (reportId: string) => api.runReport(websiteId, reportId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['reports', websiteId] });
    },
    onError: (error: Error) => {
      toast({ title: 'Failed to run report', description: error.message, variant: 'destructive' });
    },
  });
}
