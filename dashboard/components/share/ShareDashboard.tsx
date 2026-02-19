'use client';

import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { StatsRow } from '@/components/dashboard/StatsRow';
import { PageviewsChart } from '@/components/dashboard/PageviewsChart';
import { DataTable } from '@/components/dashboard/DataTable';

const BASE = typeof window !== 'undefined' ? '' : 'http://localhost:3000';

async function fetchShareStats(shareId: string) {
  const today = new Date().toISOString().slice(0, 10);
  const sevenDaysAgo = new Date(Date.now() - 6 * 86400000).toISOString().slice(0, 10);
  const res = await fetch(
    `${BASE}/api/share/${shareId}/stats?start_date=${sevenDaysAgo}&end_date=${today}`
  );
  if (!res.ok) throw new Error('Share link not found or expired');
  return res.json();
}

async function fetchSharePageviews(shareId: string) {
  const today = new Date().toISOString().slice(0, 10);
  const sevenDaysAgo = new Date(Date.now() - 6 * 86400000).toISOString().slice(0, 10);
  const res = await fetch(
    `${BASE}/api/share/${shareId}/pageviews?start_date=${sevenDaysAgo}&end_date=${today}`
  );
  if (!res.ok) return { data: { series: [], granularity: 'day' } };
  return res.json();
}

async function fetchShareMetrics(shareId: string, type: string) {
  const today = new Date().toISOString().slice(0, 10);
  const sevenDaysAgo = new Date(Date.now() - 6 * 86400000).toISOString().slice(0, 10);
  const res = await fetch(
    `${BASE}/api/share/${shareId}/metrics?type=${type}&start_date=${sevenDaysAgo}&end_date=${today}`
  );
  if (!res.ok) return { data: { type, rows: [] }, pagination: {} };
  return res.json();
}

interface ShareDashboardProps {
  shareId: string;
}

export function ShareDashboard({ shareId }: ShareDashboardProps) {
  const { data: statsData, isLoading: statsLoading, error } = useQuery({
    queryKey: ['share', shareId, 'stats'],
    queryFn: () => fetchShareStats(shareId),
    enabled: !!shareId,
    retry: false,
  });

  const { data: pvData, isLoading: pvLoading } = useQuery({
    queryKey: ['share', shareId, 'pageviews'],
    queryFn: () => fetchSharePageviews(shareId),
    enabled: !!shareId,
  });

  const { data: pagesData, isLoading: pagesLoading } = useQuery({
    queryKey: ['share', shareId, 'metrics', 'page'],
    queryFn: () => fetchShareMetrics(shareId, 'page'),
    enabled: !!shareId,
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

  const stats = statsData?.data;
  const series = pvData?.data?.series ?? [];

  return (
    <div className="min-h-screen bg-canvas">
      {/* Header */}
      <header className="border-b border-line px-6 py-4">
        <div className="max-w-5xl mx-auto flex items-center justify-between">
          <span className="text-sm font-semibold text-ink">
            spark<span className="text-spark">lytics</span>
          </span>
          <span className="text-xs text-ink-3">Read-only view</span>
        </div>
      </header>

      {/* Content */}
      <main className="max-w-5xl mx-auto px-6 py-6 space-y-6">
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
        </div>
      </main>
    </div>
  );
}
