'use client';

import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { StatsRow } from '@/components/dashboard/StatsRow';
import { PageviewsChart } from '@/components/dashboard/PageviewsChart';
import { DataTable } from '@/components/dashboard/DataTable';

const BASE = typeof window !== 'undefined' ? '' : 'http://localhost:3000';

type DatePreset = { label: string; days: number };
const DATE_PRESETS: DatePreset[] = [
  { label: 'Today', days: 0 },
  { label: '7d', days: 7 },
  { label: '30d', days: 30 },
  { label: '90d', days: 90 },
];

function getDateRange(days: number) {
  const today = new Date().toISOString().slice(0, 10);
  if (days === 0) return { start_date: today, end_date: today };
  const start = new Date(Date.now() - (days - 1) * 86400000).toISOString().slice(0, 10);
  return { start_date: start, end_date: today };
}

async function fetchShareOverview(shareId: string, days: number) {
  const { start_date, end_date } = getDateRange(days);
  const res = await fetch(
    `${BASE}/api/share/${shareId}/overview?start_date=${start_date}&end_date=${end_date}`
  );
  if (!res.ok) throw new Error('Share link not found or expired');
  return res.json();
}

interface ShareDashboardProps {
  shareId: string;
}

export function ShareDashboard({ shareId }: ShareDashboardProps) {
  const [days, setDays] = useState(7);

  const { data: overviewData, isLoading: overviewLoading, error } = useQuery({
    queryKey: ['share', shareId, 'overview', days],
    queryFn: () => fetchShareOverview(shareId, days),
    enabled: !!shareId,
    retry: false,
  });

  if (error) {
    return (
      <div className="min-h-screen bg-canvas flex items-center justify-center">
        <div className="text-center">
          <h2 className="text-base font-medium text-ink mb-2">Share link not found</h2>
          <p className="text-sm text-ink-3">This link may have been disabled or never existed.</p>
        </div>
      </div>
    );
  }

  const stats = overviewData?.data?.stats;
  const series = overviewData?.data?.pageviews?.series ?? [];
  const metrics = overviewData?.data?.metrics;

  return (
    <div className="min-h-screen bg-canvas">
      {/* Header */}
      <header className="border-b border-line px-6 py-4">
        <div className="max-w-5xl mx-auto flex items-center justify-between">
          <span className="text-sm font-semibold text-ink">
            spark<span className="text-spark">lytics</span>
          </span>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-1">
              {DATE_PRESETS.map((preset) => (
                <button
                  key={preset.days}
                  onClick={() => setDays(preset.days)}
                  className={`px-2 py-1 text-xs rounded transition-colors ${
                    days === preset.days
                      ? 'bg-surface-2 text-ink'
                      : 'text-ink-3 hover:text-ink'
                  }`}
                >
                  {preset.label}
                </button>
              ))}
            </div>
            <span className="text-xs text-ink-3">Read-only view</span>
          </div>
        </div>
      </header>

      {/* Content */}
      <main className="max-w-5xl mx-auto px-6 py-6 space-y-6">
        <StatsRow stats={stats} series={series} loading={overviewLoading} />
        <PageviewsChart data={series} loading={overviewLoading} />
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <DataTable
            title="Pages"
            filterKey="page"
            data={metrics?.page?.rows}
            loading={overviewLoading}
            showPageviews
          />
          <DataTable
            title="Referrers"
            filterKey="referrer"
            data={metrics?.referrer?.rows}
            loading={overviewLoading}
          />
          <DataTable
            title="Browsers"
            filterKey="browser"
            data={metrics?.browser?.rows}
            loading={overviewLoading}
          />
          <DataTable
            title="Countries"
            filterKey="country"
            data={metrics?.country?.rows}
            loading={overviewLoading}
          />
          <DataTable
            title="Operating Systems"
            filterKey="os"
            data={metrics?.os?.rows}
            loading={overviewLoading}
          />
          <DataTable
            title="Devices"
            filterKey="device"
            data={metrics?.device?.rows}
            loading={overviewLoading}
          />
        </div>
      </main>
    </div>
  );
}
