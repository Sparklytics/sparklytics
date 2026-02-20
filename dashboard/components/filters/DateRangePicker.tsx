'use client';

import { useState } from 'react';
import { CalendarIcon, ChevronLeft, ChevronRight } from 'lucide-react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Button } from '@/components/ui/button';
import { Calendar } from '@/components/ui/calendar';
import { useFilters } from '@/hooks/useFilters';
import { toISODate, daysAgo } from '@/lib/utils';
import { cn } from '@/lib/utils';

type Preset = { label: string; key: string; days: number };

const PRESETS: Preset[] = [
  { label: 'Today', key: 'today', days: 0 },
  { label: '24h', key: '24h', days: 1 },
  { label: '7d', key: '7d', days: 7 },
  { label: '30d', key: '30d', days: 30 },
  { label: '90d', key: '90d', days: 90 },
  { label: '12m', key: '12m', days: 365 },
];

function getPresetRange(days: number): { start_date: string; end_date: string } {
  const today = toISODate(new Date());
  if (days === 0) return { start_date: today, end_date: today };
  return { start_date: toISODate(daysAgo(days)), end_date: today };
}

function detectActivePreset(startDate: string, endDate: string): Preset | null {
  const today = toISODate(new Date());
  if (endDate !== today) return null;
  for (const p of PRESETS) {
    const range = getPresetRange(p.days);
    if (range.start_date === startDate) return p;
  }
  return null;
}

/** Shift a date range backward or forward by its own span length */
function shiftRange(
  startDate: string,
  endDate: string,
  direction: -1 | 1
): { start_date: string; end_date: string } {
  const start = new Date(startDate + 'T00:00:00');
  const end = new Date(endDate + 'T00:00:00');
  const span = Math.round((end.getTime() - start.getTime()) / 86400000) + 1;
  const shiftMs = direction * span * 86400000;
  const newStart = new Date(start.getTime() + shiftMs);
  const newEnd = new Date(end.getTime() + shiftMs);
  // Don't go into the future
  const newEndStr = toISODate(newEnd);
  const todayStr = toISODate(new Date());
  if (newEndStr > todayStr) {
    return { start_date: startDate, end_date: endDate };
  }
  return { start_date: toISODate(newStart), end_date: newEndStr };
}

function formatRangeLabel(startDate: string, endDate: string): string {
  const opts: Intl.DateTimeFormatOptions = { month: 'short', day: 'numeric' };
  const start = new Date(startDate + 'T00:00:00');
  const end = new Date(endDate + 'T00:00:00');
  if (startDate === endDate) {
    return start.toLocaleDateString('en-US', { ...opts, year: 'numeric' });
  }
  if (start.getFullYear() === end.getFullYear()) {
    return `${start.toLocaleDateString('en-US', opts)} – ${end.toLocaleDateString('en-US', opts)}`;
  }
  return `${start.toLocaleDateString('en-US', { ...opts, year: 'numeric' })} – ${end.toLocaleDateString('en-US', { ...opts, year: 'numeric' })}`;
}

export function DateRangePicker() {
  const { dateRange, setDateRange } = useFilters();
  const [calendarOpen, setCalendarOpen] = useState(false);
  const activePreset = detectActivePreset(dateRange.start_date, dateRange.end_date);
  const today = toISODate(new Date());
  const canGoForward = dateRange.end_date < today;

  function applyPreset(preset: Preset) {
    setDateRange(getPresetRange(preset.days));
    setCalendarOpen(false);
  }

  function shift(direction: -1 | 1) {
    setDateRange(shiftRange(dateRange.start_date, dateRange.end_date, direction));
  }

  return (
    <div className="flex items-center gap-1">
      {/* Prev/Next arrows */}
      <button
        onClick={() => shift(-1)}
        className="flex items-center justify-center w-8 h-8 rounded-md text-ink-3 hover:text-ink hover:bg-surface-1 transition-colors"
        aria-label="Previous period"
      >
        <ChevronLeft className="w-4 h-4" />
      </button>
      <button
        onClick={() => shift(1)}
        disabled={!canGoForward}
        className={cn(
          'flex items-center justify-center w-8 h-8 rounded-md transition-colors',
          canGoForward
            ? 'text-ink-3 hover:text-ink hover:bg-surface-1'
            : 'text-ink-4 cursor-not-allowed'
        )}
        aria-label="Next period"
      >
        <ChevronRight className="w-4 h-4" />
      </button>

      {/* Preset buttons */}
      <div className="flex items-center bg-surface-1 border border-line rounded-lg p-1 gap-0">
        {PRESETS.map((preset) => (
          <button
            key={preset.key}
            onClick={() => applyPreset(preset)}
            className={cn(
              'px-2 py-1 text-xs rounded-md transition-colors',
              activePreset?.key === preset.key
                ? 'bg-surface-2 text-ink font-medium'
                : 'text-ink-3 hover:text-ink'
            )}
          >
            {preset.label}
          </button>
        ))}

        {/* Custom (calendar) */}
        <Popover open={calendarOpen} onOpenChange={setCalendarOpen}>
          <PopoverTrigger asChild>
            <button
              className={cn(
                'flex items-center gap-1 px-2 py-1 text-xs rounded-md transition-colors',
                !activePreset
                  ? 'bg-surface-2 text-ink font-medium'
                  : 'text-ink-3 hover:text-ink'
              )}
            >
              <CalendarIcon className="w-3 h-3" />
              {activePreset ? 'Custom' : formatRangeLabel(dateRange.start_date, dateRange.end_date)}
            </button>
          </PopoverTrigger>

          <PopoverContent
            align="end"
            className="w-auto p-0 bg-surface-2 border-line-3"
          >
            <div className="flex">
              {/* Presets list in popover */}
              <div className="p-3 border-r border-line space-y-1 min-w-[140px]">
                {PRESETS.map((preset) => (
                  <button
                    key={preset.key}
                    onClick={() => applyPreset(preset)}
                    className={cn(
                      'block w-full text-left text-xs px-2 py-2 rounded-md transition-colors',
                      activePreset?.key === preset.key
                        ? 'text-ink bg-surface-1'
                        : 'text-ink-2 hover:text-ink hover:bg-canvas'
                    )}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>

              {/* Calendar */}
              <Calendar
                mode="range"
                selected={{
                  from: new Date(dateRange.start_date + 'T00:00:00'),
                  to: new Date(dateRange.end_date + 'T00:00:00'),
                }}
                onSelect={(range) => {
                  if (range?.from && range?.to) {
                    setDateRange({
                      start_date: toISODate(range.from),
                      end_date: toISODate(range.to),
                    });
                    setCalendarOpen(false);
                  }
                }}
                numberOfMonths={1}
                className="p-3"
              />
            </div>
          </PopoverContent>
        </Popover>
      </div>
    </div>
  );
}
