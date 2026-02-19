'use client';

import { useState } from 'react';
import { CalendarIcon } from 'lucide-react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Button } from '@/components/ui/button';
import { Calendar } from '@/components/ui/calendar';
import { useFilters } from '@/hooks/useFilters';
import { toISODate, daysAgo } from '@/lib/utils';
import { cn } from '@/lib/utils';

const PRESETS = [
  { label: 'Last 7 days', days: 7 },
  { label: 'Last 30 days', days: 30 },
  { label: 'Last 90 days', days: 90 },
  { label: 'Last 12 months', days: 365 },
];

export function DateRangePicker() {
  const { dateRange, setDateRange } = useFilters();
  const [open, setOpen] = useState(false);

  const activePreset = PRESETS.find(
    (p) => dateRange.start_date === toISODate(daysAgo(p.days))
  );

  function applyPreset(days: number) {
    setDateRange({
      start_date: toISODate(daysAgo(days)),
      end_date: toISODate(new Date()),
    });
    setOpen(false);
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className={cn(
            'h-8 gap-2 border-line bg-transparent text-ink-2 hover:text-ink hover:bg-surface-1 text-xs',
            'font-mono tabular-nums'
          )}
        >
          <CalendarIcon className="w-4 h-4" />
          {activePreset ? activePreset.label : `${dateRange.start_date} – ${dateRange.end_date}`}
        </Button>
      </PopoverTrigger>

      <PopoverContent
        align="end"
        className="w-auto p-0 bg-surface-2 border-line-3"
      >
        <div className="flex">
          {/* Presets */}
          <div className="p-3 border-r border-line space-y-1 min-w-[140px]">
            {PRESETS.map((preset) => (
              <button
                key={preset.days}
                onClick={() => applyPreset(preset.days)}
                className={cn(
                  'block w-full text-left text-xs px-2 py-2 rounded transition-colors',
                  activePreset?.days === preset.days
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
              from: new Date(dateRange.start_date),
              to: new Date(dateRange.end_date),
            }}
            onSelect={(range) => {
              if (range?.from && range?.to) {
                setDateRange({
                  start_date: toISODate(range.from),
                  end_date: toISODate(range.to),
                });
                setOpen(false);
              }
            }}
            numberOfMonths={1}
            className="p-3"
          />
        </div>
      </PopoverContent>
    </Popover>
  );
}
