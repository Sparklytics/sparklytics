'use client';

import { useMetrics } from './useMetrics';

export function useTopPages(websiteId: string, limit = 20): string[] {
  const { data } = useMetrics(websiteId, 'page');
  return (data?.data?.rows ?? []).slice(0, limit).map((r) => r.value);
}
