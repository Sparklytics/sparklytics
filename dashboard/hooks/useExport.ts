'use client';

import { api } from '@/lib/api';

export function useExport(websiteId: string) {
  function triggerExport(startDate: string, endDate: string) {
    const url = api.getExportUrl(websiteId, startDate, endDate);
    const a = document.createElement('a');
    a.href = url;
    a.download = `events-${websiteId}-${startDate}-${endDate}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  return { triggerExport };
}
