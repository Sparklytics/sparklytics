'use client';

import type { FunnelStepResult } from '@/lib/api';

interface FunnelBarChartProps {
  steps: FunnelStepResult[];
  totalEntered: number;
}

export function FunnelBarChart({ steps, totalEntered }: FunnelBarChartProps) {
  if (!steps.length) return null;

  return (
    <div className="space-y-3">
      {steps.map((step) => {
        const barPct = totalEntered > 0 ? (step.sessions_reached / totalEntered) * 100 : 0;
        const isFirst = step.step_order === 1;

        return (
          <div key={step.step_order} className="grid grid-cols-[1fr_auto] gap-4 items-center">
            <div>
              <div className="flex items-center gap-2 mb-1">
                <span className="text-ink-4 font-mono tabular-nums text-xs w-4 text-center shrink-0">
                  {step.step_order}
                </span>
                <span className="text-ink text-sm truncate">{step.label}</span>
              </div>
              <div className="relative h-6 bg-surface-2 rounded-sm overflow-hidden">
                <div
                  className="absolute left-0 top-0 h-full bg-spark/70 transition-all duration-500"
                  style={{ width: `${barPct}%` }}
                  role="progressbar"
                  aria-valuenow={Math.round(barPct)}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-label={`${step.label}: ${(step.conversion_rate_from_start * 100).toFixed(1)}%`}
                />
              </div>
            </div>
            <div className="text-right shrink-0 space-y-1">
              <div className="font-mono tabular-nums text-ink text-sm">
                {step.sessions_reached.toLocaleString()}
              </div>
              <div className="font-mono tabular-nums text-spark text-xs">
                {(step.conversion_rate_from_start * 100).toFixed(1)}%
              </div>
              {!isFirst && (
                <div className="font-mono tabular-nums text-down text-xs">
                  -{(step.drop_off_rate * 100).toFixed(1)}%
                </div>
              )}
            </div>
          </div>
        );
      })}

      <div className="pt-2 border-t border-line flex justify-between text-xs text-ink-3">
        <span>
          Total entered:{' '}
          <span className="font-mono tabular-nums text-ink">{totalEntered.toLocaleString()}</span>
        </span>
      </div>
    </div>
  );
}
