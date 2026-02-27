'use client';

import { useFilters } from '@/hooks/useFilters';

const inputClass =
  'h-8 px-2 text-xs bg-surface-1 border border-line rounded-md text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

export function CompareModePicker() {
  const { compare, dateRange, setCompare } = useFilters();
  const primaryStart = new Date(dateRange.start_date);
  const primaryEnd = new Date(dateRange.end_date);
  const primaryDays = Math.floor((primaryEnd.getTime() - primaryStart.getTime()) / 86_400_000) + 1;

  const customStart = compare.compare_start_date ? new Date(compare.compare_start_date) : null;
  const customEnd = compare.compare_end_date ? new Date(compare.compare_end_date) : null;
  const customDays = customStart && customEnd
    ? Math.floor((customEnd.getTime() - customStart.getTime()) / 86_400_000) + 1
    : 0;
  const invalidCustomRange =
    compare.mode === 'custom'
    && !!customStart
    && !!customEnd
    && (customEnd < customStart || customDays > primaryDays * 2);

  return (
    <div className="flex items-center gap-2">
      <select
        value={compare.mode}
        onChange={(e) => {
          const mode = e.target.value as typeof compare.mode;
          if (mode === 'custom') {
            setCompare({
              mode,
              compare_start_date: compare.compare_start_date ?? dateRange.start_date,
              compare_end_date: compare.compare_end_date ?? dateRange.end_date,
            });
            return;
          }
          setCompare({ mode });
        }}
        className={inputClass}
      >
        <option value="none">No compare</option>
        <option value="previous_period">Previous period</option>
        <option value="previous_year">Previous year</option>
        <option value="custom">Custom</option>
      </select>
      {compare.mode === 'custom' && (
        <div className="flex items-center gap-2">
          <input
            type="date"
            value={compare.compare_start_date ?? dateRange.start_date}
            max={compare.compare_end_date}
            onChange={(e) =>
              setCompare({
                ...compare,
                compare_start_date: e.target.value,
              })
            }
            className={`${inputClass} ${invalidCustomRange ? 'border-down focus:ring-down focus:border-down' : ''}`}
            aria-invalid={invalidCustomRange}
            aria-label="Compare start"
          />
          <input
            type="date"
            value={compare.compare_end_date ?? dateRange.end_date}
            min={compare.compare_start_date}
            onChange={(e) =>
              setCompare({
                ...compare,
                compare_end_date: e.target.value,
              })
            }
            className={`${inputClass} ${invalidCustomRange ? 'border-down focus:ring-down focus:border-down' : ''}`}
            aria-invalid={invalidCustomRange}
            aria-label="Compare end"
          />
          <span className={`text-[10px] ${invalidCustomRange ? 'text-down' : 'text-ink-4'}`}>
            {invalidCustomRange
              ? 'Compare range must be valid and no longer than 2x primary range.'
              : 'Custom compare dates are required.'}
          </span>
        </div>
      )}
    </div>
  );
}
