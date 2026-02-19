'use client';

import { create } from 'zustand';
import { toISODate, daysAgo } from '@/lib/utils';

export type DateRange = {
  start_date: string;
  end_date: string;
};

// Filters are stored with the "filter_" prefix so they can be spread
// directly into API call params (e.g. { filter_country: 'PL' }).
export type Filters = Record<string, string>;

interface FiltersState {
  dateRange: DateRange;
  filters: Filters;
  setDateRange: (range: DateRange) => void;
  // key: base name without "filter_" prefix (e.g. "country", "page")
  setFilter: (key: string, value: string) => void;
  removeFilter: (key: string) => void;
  clearFilters: () => void;
}

function defaultDateRange(): DateRange {
  return {
    start_date: toISODate(daysAgo(30)),
    end_date: toISODate(new Date()),
  };
}

function readFromUrl(): { dateRange: DateRange; filters: Filters } {
  if (typeof window === 'undefined') {
    return { dateRange: defaultDateRange(), filters: {} };
  }
  const params = new URLSearchParams(window.location.search);
  const dateRange: DateRange = {
    start_date: params.get('start') ?? toISODate(daysAgo(30)),
    end_date: params.get('end') ?? toISODate(new Date()),
  };
  const filters: Filters = {};
  for (const [key, value] of params.entries()) {
    // Only read params with "filter_" prefix; store them as-is.
    if (key.startsWith('filter_')) {
      filters[key] = value;
    }
  }
  return { dateRange, filters };
}

function writeToUrl(dateRange: DateRange, filters: Filters): void {
  if (typeof window === 'undefined') return;
  const params = new URLSearchParams();
  params.set('start', dateRange.start_date);
  params.set('end', dateRange.end_date);
  for (const [key, value] of Object.entries(filters)) {
    params.set(key, value); // keys already include "filter_" prefix
  }
  history.replaceState(null, '', `${window.location.pathname}?${params.toString()}`);
}

const initial = readFromUrl();

export const useFilters = create<FiltersState>((set) => ({
  dateRange: initial.dateRange,
  filters: initial.filters,

  setDateRange: (range) =>
    set((state) => {
      writeToUrl(range, state.filters);
      return { dateRange: range };
    }),

  setFilter: (key, value) =>
    set((state) => {
      const filters = { ...state.filters, [`filter_${key}`]: value };
      writeToUrl(state.dateRange, filters);
      return { filters };
    }),

  removeFilter: (key) =>
    set((state) => {
      const next = { ...state.filters };
      delete next[`filter_${key}`];
      writeToUrl(state.dateRange, next);
      return { filters: next };
    }),

  clearFilters: () =>
    set((state) => {
      writeToUrl(state.dateRange, {});
      return { filters: {} };
    }),
}));
