'use client';

import { useEffect, useState } from 'react';
import { DateRangePicker } from '@/components/filters/DateRangePicker';
import { FilterBar } from '@/components/filters/FilterBar';
import { ExportButton } from '@/components/dashboard/ExportButton';
import { useFilters } from '@/hooks/useFilters';

function useWebsiteIdFromUrl(): string {
  const [id, setId] = useState('');
  useEffect(() => {
    function read() {
      const segs = window.location.pathname.split('/').filter(Boolean);
      // /dashboard/<websiteId>/...
      const idx = segs.indexOf('dashboard');
      setId(idx !== -1 ? (segs[idx + 1] ?? '') : '');
    }
    read();
    window.addEventListener('popstate', read);
    return () => window.removeEventListener('popstate', read);
  }, []);
  return id;
}

export function Header() {
  const websiteId = useWebsiteIdFromUrl();
  const { dateRange } = useFilters();

  return (
    <header className="h-14 border-b border-line flex items-center gap-4 px-6 shrink-0">
      <div className="flex-1" />
      <FilterBar />
      {websiteId && (
        <ExportButton
          websiteId={websiteId}
          startDate={dateRange.start_date}
          endDate={dateRange.end_date}
        />
      )}
      <DateRangePicker />
    </header>
  );
}
