'use client';

import { DateRangePicker } from '@/components/filters/DateRangePicker';
import { FilterBar } from '@/components/filters/FilterBar';

export function Header() {
  return (
    <header className="h-14 border-b border-line flex items-center gap-4 px-6 shrink-0">
      <div className="flex-1" />
      <FilterBar />
      <DateRangePicker />
    </header>
  );
}
