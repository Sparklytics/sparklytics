'use client';

import { useEffect, useState } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { StatsRow } from '@/components/dashboard/StatsRow';
import { PageviewsChart } from '@/components/dashboard/PageviewsChart';
import { DataTable } from '@/components/dashboard/DataTable';
import { RealtimePanel } from '@/components/dashboard/RealtimePanel';
import { EmptyState } from '@/components/dashboard/EmptyState';
import { WorldMap } from '@/components/dashboard/WorldMap';
import { RealtimePage } from '@/components/realtime/RealtimePage';
import { WebsiteDetail } from '@/components/settings/WebsiteDetail';
import { EventsPage } from '@/components/events/EventsPage';
import { SessionsPage } from '@/components/sessions/SessionsPage';
import { GoalsPage } from '@/components/goals/GoalsPage';
import { useStats } from '@/hooks/useStats';
import { usePageviews } from '@/hooks/usePageviews';
import { useMetrics } from '@/hooks/useMetrics';
import { useRealtime } from '@/hooks/useRealtime';
import { useAuth } from '@/hooks/useAuth';
import { useWebsites } from '@/hooks/useWebsites';
import { cn } from '@/lib/utils';



function navigate(path: string) {
  window.history.pushState({}, '', path);
  window.dispatchEvent(new PopStateEvent('popstate'));
}

// For the static export SPA: the Rust server serves this shell for all
// /dashboard/* paths. We read the websiteId from window.location client-side.
function useUrlSegments(): { websiteId: string; subPage: string; subSubPage: string } {
  const [websiteId, setWebsiteId] = useState('');
  const [subPage, setSubPage] = useState('');
  const [subSubPage, setSubSubPage] = useState('');
  useEffect(() => {
    function read() {
      // URL pattern: /dashboard/<websiteId>/<subPage>
      const segments = window.location.pathname.split('/').filter(Boolean);
      setWebsiteId(segments[1] ?? '');
      setSubPage(segments[2] ?? '');
      setSubSubPage(segments[3] ?? '');
    }
    read();
    window.addEventListener('popstate', read);
    return () => window.removeEventListener('popstate', read);
  }, []);
  return { websiteId, subPage, subSubPage };
}

export function DashboardClient() {
  const { websiteId, subPage, subSubPage } = useUrlSegments();
  const { data: authStatus, isSuccess: authLoaded, isError: authError } = useAuth();
  const { data: websitesData } = useWebsites();
  const analyticsEnabled = subPage !== 'settings' && subPage !== 'realtime'
    && subPage !== 'sessions' && subPage !== 'goals';

  // Auth redirect guard
  useEffect(() => {
    if (!authLoaded || authError) return;
    if (authStatus === null) return; // mode=none, open access
    if (authStatus.setup_required) {
      window.location.href = '/setup';
      return;
    }
    if (!authStatus.authenticated) {
      window.location.href = '/login';
    }
  }, [authStatus, authLoaded, authError]);

  // Redirect bare /dashboard (no websiteId) → first website or /onboarding
  useEffect(() => {
    if (websiteId || !websitesData) return;
    if (websitesData.data.length > 0) {
      navigate(`/dashboard/${websitesData.data[0].id}`);
    } else {
      navigate('/onboarding');
    }
  }, [websiteId, websitesData]);

  // Find the current website for EmptyState domain prop
  const currentWebsite = websitesData?.data?.find((w) => w.id === websiteId);

  const { data: statsData, isLoading: statsLoading } = useStats(websiteId, analyticsEnabled);
  const { data: pageviewsData, isLoading: pvLoading } = usePageviews(websiteId, analyticsEnabled);
  const { data: pagesData, isLoading: pagesLoading } = useMetrics(websiteId, 'page', analyticsEnabled);
  const { data: referrersData, isLoading: refLoading } = useMetrics(websiteId, 'referrer', analyticsEnabled);
  const { data: browsersData, isLoading: browsersLoading } = useMetrics(websiteId, 'browser', analyticsEnabled);
  const { data: countriesData, isLoading: countriesLoading } = useMetrics(websiteId, 'country', analyticsEnabled);
  const { data: osData, isLoading: osLoading } = useMetrics(websiteId, 'os', analyticsEnabled);
  const { data: devicesData, isLoading: devicesLoading } = useMetrics(websiteId, 'device', analyticsEnabled);
  const { data: realtimeData, isLoading: rtLoading } = useRealtime(websiteId, 30_000, analyticsEnabled);

  // Settings subpage: render inline
  if (subPage === 'settings') {
    return (
      <AppShell websiteId={websiteId}>
        <WebsiteDetail websiteId={websiteId} subSubPage={subSubPage || 'general'} />
      </AppShell>
    );
  }

  // Realtime subpage: dedicated full-screen view
  if (subPage === 'realtime') {
    return (
      <AppShell websiteId={websiteId}>
        <RealtimePage websiteId={websiteId} />
      </AppShell>
    );
  }

  // Sessions subpage: full-screen sessions explorer
  if (subPage === 'sessions') {
    return (
      <AppShell websiteId={websiteId}>
        <SessionsPage websiteId={websiteId} />
      </AppShell>
    );
  }

  // Goals subpage: goals management and conversion tracking
  if (subPage === 'goals') {
    return (
      <AppShell websiteId={websiteId}>
        <GoalsPage websiteId={websiteId} />
      </AppShell>
    );
  }

  const stats = statsData?.data;
  const series = pageviewsData?.data?.series ?? [];
  const isEmpty = !statsLoading && stats && stats.pageviews === 0;

  return (
    <AppShell websiteId={websiteId}>
      {isEmpty ? (
        <EmptyState websiteId={websiteId} domain={currentWebsite?.domain} />
      ) : (
        <div className="space-y-6">
          <StatsRow stats={stats} series={series} loading={statsLoading || pvLoading} />
          <PageviewsChart data={series} loading={pvLoading} />

          {(!subPage || subPage === 'overview') && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              <DataTable
                title="Top Pages"
                filterKey="page"
                data={pagesData?.data?.rows}
                loading={pagesLoading}
                showPageviews
              />
              <DataTable
                title="Top Referrers"
                filterKey="referrer"
                data={referrersData?.data?.rows}
                loading={refLoading}
              />
            </div>
          )}

          {subPage === 'pages' && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              <DataTable
                title="Pages"
                filterKey="page"
                data={pagesData?.data?.rows}
                loading={pagesLoading}
                showPageviews
              />
              {/* Detailed view coming here */}
            </div>
          )}

          {subPage === 'geolocation' && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 items-start">
              <WorldMap data={countriesData?.data?.rows} loading={countriesLoading} />
              <DataTable
                title="Countries"
                filterKey="country"
                data={countriesData?.data?.rows}
                loading={countriesLoading}
              />
            </div>
          )}

          {subPage === 'systems' && (
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
              <DataTable
                title="Browsers"
                filterKey="browser"
                data={browsersData?.data?.rows}
                loading={browsersLoading}
              />
              <DataTable
                title="Operating Systems"
                filterKey="os"
                data={osData?.data?.rows}
                loading={osLoading}
              />
              <DataTable
                title="Devices"
                filterKey="device"
                data={devicesData?.data?.rows}
                loading={devicesLoading}
              />
            </div>
          )}

          {subPage === 'events' && <EventsPage websiteId={websiteId} />}

          <RealtimePanel data={realtimeData?.data} loading={rtLoading} />
        </div>
      )}
    </AppShell>
  );
}
