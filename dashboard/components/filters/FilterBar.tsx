'use client';

import { useFilters } from '@/hooks/useFilters';
import { FilterChip } from './FilterChip';

// Maps base filter key (without "filter_" prefix) to a human-readable label.
const FILTER_LABELS: Record<string, string> = {
  page: 'Page',
  referrer: 'Referrer',
  browser: 'Browser',
  os: 'OS',
  device: 'Device',
  country: 'Country',
};

export function FilterBar() {
  const { filters, removeFilter } = useFilters();
  const entries = Object.entries(filters);

  if (entries.length === 0) return null;

  return (
    <div className="flex items-center gap-2 flex-wrap">
      {entries.map(([key, value]) => {
        // Filters are stored with "filter_" prefix; strip it for display/removal.
        const baseKey = key.startsWith('filter_') ? key.slice('filter_'.length) : key;
        return (
          <FilterChip
            key={key}
            label={FILTER_LABELS[baseKey] ?? baseKey}
            value={value}
            onRemove={() => removeFilter(baseKey)}
          />
        );
      })}
    </div>
  );
}
