'use client';

import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api, AttributionModel, AttributionParams } from '@/lib/api';
import { useFilters } from './useFilters';

function buildParams(
  goalId: string,
  model: AttributionModel,
  dateRange: { start_date: string; end_date: string },
  filters: Record<string, string>
): AttributionParams {
  return {
    goal_id: goalId,
    model,
    start_date: dateRange.start_date,
    end_date: dateRange.end_date,
    ...(filters as Partial<AttributionParams>),
  };
}

export function useAttribution(
  websiteId: string,
  goalId: string,
  model: AttributionModel
) {
  const { dateRange, filters } = useFilters();

  const params = useMemo(
    () => buildParams(goalId, model, dateRange, filters),
    [dateRange, filters, goalId, model]
  );

  return useQuery({
    queryKey: ['attribution', websiteId, params],
    queryFn: () => api.getAttribution(websiteId, params),
    enabled: !!websiteId && !!goalId,
    staleTime: 60_000,
    placeholderData: (previousData) => previousData,
  });
}

export function useRevenueSummary(
  websiteId: string,
  goalId: string,
  model: AttributionModel
) {
  const { dateRange, filters } = useFilters();

  const params = useMemo(
    () => buildParams(goalId, model, dateRange, filters),
    [dateRange, filters, goalId, model]
  );

  return useQuery({
    queryKey: ['revenue-summary', websiteId, params],
    queryFn: () => api.getRevenueSummary(websiteId, params),
    enabled: !!websiteId && !!goalId,
    staleTime: 60_000,
    placeholderData: (previousData) => previousData,
  });
}
