'use client';

import { useFunnelResults } from '@/hooks/useFunnelResults';
import { FunnelBarChart } from './FunnelBarChart';

interface FunnelResultsPanelProps {
  websiteId: string;
  funnelId: string;
}

export function FunnelResultsPanel({ websiteId, funnelId }: FunnelResultsPanelProps) {
  const { data, isLoading, error } = useFunnelResults(websiteId, funnelId);

  if (isLoading) {
    return (
      <div className="space-y-3 pt-4">
        {[1, 2, 3].map((i) => (
          <div key={i} className="h-8 bg-surface-2 rounded animate-pulse" />
        ))}
      </div>
    );
  }

  if (error) {
    console.error('[FunnelResultsPanel]', error);
    return (
      <p className="pt-4 text-xs text-down">
        Failed to load funnel results. Try refreshing.
      </p>
    );
  }

  if (!data?.data) {
    return (
      <div className="pt-4 py-8 text-center">
        <p className="text-sm font-medium text-ink">No matching sessions</p>
        <p className="text-xs text-ink-3 mt-1">
          Try widening the date range or adjusting the steps.
        </p>
      </div>
    );
  }

  const results = data.data;

  return (
    <div className="pt-4 space-y-6">
      {/* Header row */}
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-xs text-ink-3">
            <span className="font-mono tabular-nums text-ink">
              {results.total_sessions_entered.toLocaleString()}
            </span>{' '}sessions entered
          </p>
        </div>
        <div className="text-right shrink-0">
          <div className="font-mono tabular-nums text-spark text-2xl font-semibold leading-none">
            {(results.final_conversion_rate * 100).toFixed(1)}%
          </div>
          <div className="text-ink-3 text-xs mt-1">final conversion</div>
        </div>
      </div>

      <FunnelBarChart steps={results.steps} totalEntered={results.total_sessions_entered} />

      {/* Detail table */}
      <div className="overflow-x-auto">
        <table className="min-w-[520px] w-full text-sm" aria-label="Funnel step breakdown">
          <thead>
            <tr className="border-b border-line text-xs font-medium text-ink-3 uppercase tracking-wider">
              <th className="text-left py-2 font-normal pr-3">#</th>
              <th className="text-left py-2 font-normal">Step</th>
              <th className="text-right py-2 font-normal pl-3">Reached</th>
              <th className="text-right py-2 font-normal pl-3">Drop-off</th>
              <th className="text-right py-2 font-normal pl-3">Conv. prev</th>
              <th className="text-right py-2 font-normal pl-3">Conv. start</th>
            </tr>
          </thead>
          <tbody>
            {results.steps.map((step) => (
              <tr key={step.step_order} className="border-b border-line/50">
                <td className="py-2 font-mono tabular-nums text-ink-3 pr-3">{step.step_order}</td>
                <td className="py-2 text-ink">{step.label}</td>
                <td className="py-2 text-right font-mono tabular-nums text-ink pl-3">
                  {step.sessions_reached.toLocaleString()}
                </td>
                <td className="py-2 text-right font-mono tabular-nums text-down pl-3">
                  {step.drop_off_count > 0 ? `-${step.drop_off_count.toLocaleString()}` : '—'}
                </td>
                <td className="py-2 text-right font-mono tabular-nums text-ink-2 pl-3">
                  {(step.conversion_rate_from_previous * 100).toFixed(1)}%
                </td>
                <td className="py-2 text-right font-mono tabular-nums text-spark pl-3">
                  {(step.conversion_rate_from_start * 100).toFixed(1)}%
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
