'use client';

import { useMemo, useState } from 'react';
import { RetentionGranularity } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';
import { useRetention } from '@/hooks/useRetention';
import { RetentionControls } from './RetentionControls';
import { RetentionSummaryCards } from './RetentionSummaryCards';
import { RetentionHeatmap } from './RetentionHeatmap';

interface RetentionPageProps {
  websiteId: string;
}

const DEFAULT_PERIODS: Record<RetentionGranularity, number> = {
  day: 30,
  week: 8,
  month: 12,
};

const MAX_PERIODS: Record<RetentionGranularity, number> = {
  day: 30,
  week: 12,
  month: 12,
};

export function RetentionPage({ websiteId }: RetentionPageProps) {
  const { dateRange } = useFilters();
  const [granularity, setGranularity] = useState<RetentionGranularity>('week');
  const [maxPeriods, setMaxPeriods] = useState<number>(DEFAULT_PERIODS.week);

  const controls = useMemo(
    () => ({
      cohort_granularity: granularity,
      max_periods: maxPeriods,
    }),
    [granularity, maxPeriods]
  );

  const { data, isLoading, isFetching, error } = useRetention(websiteId, controls);
  const result = data?.data;

  function handleGranularityChange(next: RetentionGranularity) {
    setGranularity(next);
    // Reset to the sensible default for the new granularity rather than
    // clamping the previous value to the ceiling.
    setMaxPeriods(DEFAULT_PERIODS[next]);
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">Retention</h2>
        <p className="text-xs text-ink-3 mt-1">
          Cohort retention by day, week, or month.
        </p>
      </div>

      <RetentionControls
        granularity={granularity}
        maxPeriods={maxPeriods}
        onGranularityChange={handleGranularityChange}
        onMaxPeriodsChange={setMaxPeriods}
      />

      {isLoading && !result ? (
        <div className="space-y-2">
          <div className="h-20 border border-line rounded-lg bg-surface-1 animate-pulse" />
          <div className="h-64 border border-line rounded-lg bg-surface-1 animate-pulse" />
        </div>
      ) : error ? (
        <div className="border border-line rounded-lg bg-surface-1 px-4 py-6">
          <p className="text-sm text-down">Failed to load retention data. Try refreshing.</p>
        </div>
      ) : result ? (
        <div className={`space-y-3 transition-opacity ${isFetching ? 'opacity-60' : ''}`}>
          <div className="border border-line rounded-lg bg-surface-1 px-4 py-3 flex flex-wrap items-center gap-2 justify-between">
            <p className="text-xs text-ink-2">
              <span className="font-mono tabular-nums text-ink">
                {result.rows.length.toLocaleString()}
              </span>{' '}
              cohorts
            </p>
            <p className="text-xs text-ink-3 font-mono tabular-nums">
              {dateRange.start_date} → {dateRange.end_date}
            </p>
          </div>

          <RetentionSummaryCards
            granularity={result.granularity}
            summary={result.summary}
          />

          <RetentionHeatmap
            rows={result.rows}
            granularity={result.granularity}
            maxPeriods={result.max_periods}
            endDate={dateRange.end_date}
          />
        </div>
      ) : null}
    </div>
  );
}
