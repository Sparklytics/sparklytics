'use client';

import { useState } from 'react';
import { AttributionModel } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';
import { useGoals } from '@/hooks/useGoals';
import { useAttribution, useRevenueSummary } from '@/hooks/useAttribution';
import { AttributionModelToggle } from './AttributionModelToggle';
import { RevenueSummaryCards } from './RevenueSummaryCards';
import { AttributionTable } from './AttributionTable';

interface AttributionPageProps {
  websiteId: string;
}

const selectClass =
  'w-full md:w-[320px] px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink focus:outline-none focus:ring-1 focus:ring-spark focus:border-spark';

export function AttributionPage({ websiteId }: AttributionPageProps) {
  const { dateRange } = useFilters();
  const { data: goalsData, isLoading: goalsLoading } = useGoals(websiteId);
  const goals = goalsData?.data ?? [];

  const defaultGoalId = goals[0]?.id ?? '';
  const [selectedGoalId, setSelectedGoalId] = useState('');
  const [model, setModel] = useState<AttributionModel>('last_touch');

  const activeGoalId = selectedGoalId || defaultGoalId;
  const activeGoal = goals.find((goal) => goal.id === activeGoalId) ?? null;

  const attributionQuery = useAttribution(websiteId, activeGoalId, model);
  const summaryQuery = useRevenueSummary(websiteId, activeGoalId, model);

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">Attribution</h2>
        <p className="text-xs text-ink-3 mt-1">
          Compare first-touch and last-touch conversion credit by channel.
        </p>
      </div>

      <div className="border border-line rounded-lg bg-surface-1 p-3 flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div className="space-y-1">
          <label htmlFor="goal-select" className="text-xs text-ink-3">
            Goal
          </label>
          <select
            id="goal-select"
            className={selectClass}
            value={activeGoalId}
            disabled={goalsLoading || goals.length === 0}
            onChange={(event) => setSelectedGoalId(event.target.value)}
          >
            {goals.length === 0 ? (
              <option value="">No goals available</option>
            ) : (
              goals.map((goal) => (
                <option key={goal.id} value={goal.id}>
                  {goal.name}
                </option>
              ))
            )}
          </select>
        </div>
        <p className="text-xs text-ink-3 font-mono tabular-nums">
          {dateRange.start_date} → {dateRange.end_date}
        </p>
      </div>

      {goals.length === 0 ? (
        <div className="border border-line rounded-lg bg-surface-1 px-6 py-10 text-center">
          <p className="text-sm font-medium text-ink mb-1">No goals to attribute yet</p>
          <p className="text-xs text-ink-3">Create a goal first, then run attribution by channel.</p>
        </div>
      ) : (
        <>
          <AttributionModelToggle model={model} onChange={setModel} />

          {summaryQuery.isError || attributionQuery.isError ? (
            <div className="border border-line rounded-lg bg-surface-1 px-4 py-6">
              <p className="text-sm text-red-400">
                Failed to load attribution data. Try refreshing.
              </p>
            </div>
          ) : (
            <>
              {summaryQuery.data?.data && (
                <RevenueSummaryCards
                  summary={summaryQuery.data.data}
                  goalName={activeGoal?.name}
                />
              )}
              <AttributionTable
                rows={attributionQuery.data?.data.rows ?? []}
                loading={attributionQuery.isLoading}
              />
            </>
          )}
        </>
      )}
    </div>
  );
}
