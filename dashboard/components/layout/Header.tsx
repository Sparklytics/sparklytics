'use client';

import { useEffect, useState } from 'react';
import { Menu } from 'lucide-react';
import { DateRangePicker } from '@/components/filters/DateRangePicker';
import { CompareModePicker } from '@/components/filters/CompareModePicker';
import { FilterBar } from '@/components/filters/FilterBar';
import { ExportButton } from '@/components/dashboard/ExportButton';
import { useFilters } from '@/hooks/useFilters';
import { useWebsites } from '@/hooks/useWebsites';
import { useRealtime } from '@/hooks/useRealtime';

function useUrlSegments(): { websiteId: string; subPage: string } {
  const [websiteId, setWebsiteId] = useState('');
  const [subPage, setSubPage] = useState('');
  useEffect(() => {
    function read() {
      const segs = window.location.pathname.split('/').filter(Boolean);
      if (segs[0] === 'dashboard') {
        setWebsiteId(segs[1] ?? '');
        setSubPage(segs[2] ?? '');
      } else if (segs[0] === 'settings') {
        setWebsiteId(segs[1] ?? '');
        setSubPage('settings');
      } else {
        setWebsiteId('');
        setSubPage('');
      }
    }
    read();
    window.addEventListener('popstate', read);
    return () => window.removeEventListener('popstate', read);
  }, []);
  return { websiteId, subPage };
}

const SUB_PAGE_LABELS: Record<string, string> = {
  '': 'Analytics',
  overview: 'Analytics',
  pages: 'Pages',
  geolocation: 'Geolocation',
  systems: 'Systems',
  events: 'Events',
  sessions: 'Sessions',
  goals: 'Goals',
  funnels: 'Funnels',
  journey: 'Journey',
  retention: 'Retention',
  attribution: 'Attribution',
  realtime: 'Realtime',
  settings: 'Settings',
};

export function Header({ onMenuClick }: { onMenuClick?: () => void }) {
  const { websiteId, subPage } = useUrlSegments();
  const { dateRange } = useFilters();
  const { data: websitesData } = useWebsites();
  const { data: realtimeData } = useRealtime(websiteId);

  const website = websitesData?.data?.find((w) => w.id === websiteId);
  const pagePart = SUB_PAGE_LABELS[subPage] ?? '';
  const title = website?.name
    ? pagePart ? `${website.name} — ${pagePart}` : website.name
    : pagePart;

  const activeVisitors = realtimeData?.data?.active_visitors ?? 0;

  return (
    <header className="h-14 border-b border-line flex items-center gap-4 px-4 md:px-6 shrink-0">
      {/* Hamburger — mobile only */}
      <button
        onClick={onMenuClick}
        className="md:hidden p-1.5 text-ink-3 hover:text-ink hover:bg-surface-1 rounded-md transition-colors"
        aria-label="Open menu"
      >
        <Menu className="w-5 h-5" />
      </button>

      {title && (
        <div className="flex items-center gap-3">
          <h1 className="text-sm font-medium text-ink truncate max-w-[200px]">{title}</h1>
          {websiteId && activeVisitors > 0 && (
            <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full border border-line bg-surface-1" title={`${activeVisitors} current active visitors`}>
              <div className="w-1.5 h-1.5 rounded-full bg-spark animate-[pulse_2s_cubic-bezier(0.4,0,0.6,1)_infinite]" />
              <span className="text-[10px] font-medium text-ink-2">{activeVisitors} active</span>
            </div>
          )}
        </div>
      )}
      <div className="flex-1" />
      <FilterBar />
      {websiteId && subPage !== 'settings' && (
        <ExportButton
          websiteId={websiteId}
          startDate={dateRange.start_date}
          endDate={dateRange.end_date}
        />
      )}
      {subPage !== 'settings' && <CompareModePicker />}
      {subPage !== 'settings' && <DateRangePicker />}
    </header>
  );
}
