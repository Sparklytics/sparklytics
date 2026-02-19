'use client';

import { useEffect, useState } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { StatsRow } from '@/components/dashboard/StatsRow';
import { PageviewsChart } from '@/components/dashboard/PageviewsChart';
import { DataTable } from '@/components/dashboard/DataTable';
import { RealtimePanel } from '@/components/dashboard/RealtimePanel';
import { EmptyState } from '@/components/dashboard/EmptyState';
import { useStats } from '@/hooks/useStats';
import { usePageviews } from '@/hooks/usePageviews';
import { useMetrics } from '@/hooks/useMetrics';
import { useRealtime } from '@/hooks/useRealtime';
import { useAuth } from '@/hooks/useAuth';

// For the static export SPA: the Rust server serves this shell for all
// /dashboard/* paths. We read the websiteId from window.location client-side.
function useWebsiteIdFromUrl(): string {
  const [websiteId, setWebsiteId] = useState('');
  useEffect(() => {
    function read() {
      // URL pattern: /dashboard/<websiteId>
      const segments = window.location.pathname.split('/').filter(Boolean);
      setWebsiteId(segments[1] ?? '');
    }
    read();
    window.addEventListener('popstate', read);
    return () => window.removeEventListener('popstate', read);
  }, []);
  return websiteId;
}

export function DashboardClient() {
  const websiteId = useWebsiteIdFromUrl();
  const { data: authStatus, isSuccess: authLoaded, isError: authError } = useAuth();

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

  const { data: statsData, isLoading: statsLoading } = useStats(websiteId);
  const { data: pageviewsData, isLoading: pvLoading } = usePageviews(websiteId);
  const { data: pagesData, isLoading: pagesLoading } = useMetrics(websiteId, 'page');
  const { data: referrersData, isLoading: refLoading } = useMetrics(websiteId, 'referrer');
  const { data: browsersData, isLoading: browsersLoading } = useMetrics(websiteId, 'browser');
  const { data: countriesData, isLoading: countriesLoading } = useMetrics(websiteId, 'country');
  const { data: osData, isLoading: osLoading } = useMetrics(websiteId, 'os');
  const { data: devicesData, isLoading: devicesLoading } = useMetrics(websiteId, 'device');
  const { data: realtimeData, isLoading: rtLoading } = useRealtime(websiteId);

  const stats = statsData?.data;
  const series = pageviewsData?.data?.series ?? [];
  const isEmpty = !statsLoading && stats && stats.pageviews === 0;

  return (
    <AppShell websiteId={websiteId}>
      {isEmpty ? (
        <EmptyState websiteId={websiteId} />
      ) : (
        <div className="space-y-6">
          <StatsRow stats={stats} series={series} loading={statsLoading || pvLoading} />
          <PageviewsChart data={series} loading={pvLoading} />

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <DataTable
              title="Pages"
              filterKey="page"
              data={pagesData?.data?.rows}
              loading={pagesLoading}
              showPageviews
            />
            <DataTable
              title="Referrers"
              filterKey="referrer"
              data={referrersData?.data?.rows}
              loading={refLoading}
            />
            <DataTable
              title="Browsers"
              filterKey="browser"
              data={browsersData?.data?.rows}
              loading={browsersLoading}
            />
            <DataTable
              title="Countries"
              filterKey="country"
              data={countriesData?.data?.rows}
              loading={countriesLoading}
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

          <RealtimePanel data={realtimeData?.data} loading={rtLoading} />
        </div>
      )}
    </AppShell>
  );
}
