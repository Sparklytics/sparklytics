'use client';

import { RetentionGranularity } from '@/lib/api';

interface RetentionControlsProps {
  granularity: RetentionGranularity;
  maxPeriods: number;
  onGranularityChange: (value: RetentionGranularity) => void;
  onMaxPeriodsChange: (value: number) => void;
}

const PERIOD_LIMITS: Record<RetentionGranularity, number> = {
  day: 30,
  week: 12,
  month: 12,
};

export function RetentionControls({
  granularity,
  maxPeriods,
  onGranularityChange,
  onMaxPeriodsChange,
}: RetentionControlsProps) {
  const maxAllowed = PERIOD_LIMITS[granularity];
  const label = granularity === 'day' ? 'Days' : granularity === 'week' ? 'Weeks' : 'Months';

  return (
    <div className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-ink-3">Granularity</span>
        <div className="flex items-center gap-1" role="group" aria-label="Retention granularity">
          {(['day', 'week', 'month'] as RetentionGranularity[]).map((option) => (
            <button
              key={option}
              type="button"
              aria-pressed={granularity === option}
              onClick={() => onGranularityChange(option)}
              className={`px-2 py-1 rounded-sm border text-xs transition-colors ${
                granularity === option
                  ? 'border-spark text-ink bg-spark/10'
                  : 'border-line text-ink-3 hover:text-ink-2 hover:border-ink-4'
              }`}
            >
              {option}
            </button>
          ))}
        </div>
      </div>

      <div className="flex items-center gap-2">
        <label htmlFor="retention-periods" className="text-xs text-ink-3">
          {label}
        </label>
        <select
          id="retention-periods"
          value={String(maxPeriods)}
          onChange={(event) => onMaxPeriodsChange(Number(event.target.value))}
          className="h-8 w-20 bg-surface-input border border-line rounded-sm px-2 text-xs text-ink"
        >
          {Array.from({ length: maxAllowed }, (_, idx) => idx + 1).map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
