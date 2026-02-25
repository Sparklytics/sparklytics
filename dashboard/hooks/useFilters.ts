'use client';

import { create } from 'zustand';
import type { CompareMode, CompareParams } from '@/lib/api';
import { toISODate, daysAgo } from '@/lib/utils';

export type DateRange = {
  start_date: string;
  end_date: string;
};

// Filters are stored with the "filter_" prefix so they can be spread
// directly into API call params (e.g. { filter_country: 'PL' }).
export type Filters = Record<string, string>;
export type CompareState = {
  mode: CompareMode;
  compare_start_date?: string;
  compare_end_date?: string;
};

export function toCompareParams(compare: CompareState): CompareParams {
  if (compare.mode === 'custom') {
    return {
      compare_mode: compare.mode,
      compare_start_date: compare.compare_start_date,
      compare_end_date: compare.compare_end_date,
    };
  }

  return { compare_mode: compare.mode };
}

interface FiltersState {
  dateRange: DateRange;
  filters: Filters;
  compare: CompareState;
  setDateRange: (range: DateRange) => void;
  // key: base name without "filter_" prefix (e.g. "country", "page")
  setFilter: (key: string, value: string) => void;
  removeFilter: (key: string) => void;
  clearFilters: () => void;
  setCompare: (compare: CompareState) => void;
}

function defaultDateRange(): DateRange {
  return {
    start_date: toISODate(daysAgo(30)),
    end_date: toISODate(new Date()),
  };
}

function readFromUrl(): { dateRange: DateRange; filters: Filters; compare: CompareState } {
  if (typeof window === 'undefined') {
    return { dateRange: defaultDateRange(), filters: {}, compare: { mode: 'none' } };
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
  const mode = (params.get('compare_mode') as CompareMode | null) ?? 'none';
  const compare: CompareState = {
    mode: ['none', 'previous_period', 'previous_year', 'custom'].includes(mode)
      ? mode
      : 'none',
    compare_start_date: params.get('compare_start') ?? undefined,
    compare_end_date: params.get('compare_end') ?? undefined,
  };
  return { dateRange, filters, compare };
}

function writeToUrl(dateRange: DateRange, filters: Filters, compare: CompareState): void {
  if (typeof window === 'undefined') return;
  const params = new URLSearchParams();
  params.set('start', dateRange.start_date);
  params.set('end', dateRange.end_date);
  for (const [key, value] of Object.entries(filters)) {
    params.set(key, value); // keys already include "filter_" prefix
  }
  if (compare.mode !== 'none') {
    params.set('compare_mode', compare.mode);
    if (compare.mode === 'custom') {
      if (compare.compare_start_date) params.set('compare_start', compare.compare_start_date);
      if (compare.compare_end_date) params.set('compare_end', compare.compare_end_date);
    }
  }
  history.replaceState(null, '', `${window.location.pathname}?${params.toString()}`);
}

const initial = readFromUrl();

export const useFilters = create<FiltersState>((set) => ({
  dateRange: initial.dateRange,
  filters: initial.filters,
  compare: initial.compare,

  setDateRange: (range) =>
    set((state) => {
      writeToUrl(range, state.filters, state.compare);
      return { dateRange: range };
    }),

  setFilter: (key, value) =>
    set((state) => {
      const filters = { ...state.filters, [`filter_${key}`]: value };
      writeToUrl(state.dateRange, filters, state.compare);
      return { filters };
    }),

  removeFilter: (key) =>
    set((state) => {
      const next = { ...state.filters };
      delete next[`filter_${key}`];
      writeToUrl(state.dateRange, next, state.compare);
      return { filters: next };
    }),

  clearFilters: () =>
    set((state) => {
      writeToUrl(state.dateRange, {}, state.compare);
      return { filters: {} };
    }),

  setCompare: (compare) =>
    set((state) => {
      writeToUrl(state.dateRange, state.filters, compare);
      return { compare };
    }),
}));
