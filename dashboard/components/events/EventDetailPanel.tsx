'use client';

import { X } from 'lucide-react';
import { useEventProperties } from '@/hooks/useEventProperties';
import { useEventTimeseries } from '@/hooks/useEventTimeseries';
import { EventPropertyTable } from './EventPropertyTable';
import { Sparkline } from '@/components/dashboard/Sparkline';

interface EventDetailPanelProps {
  websiteId: string;
  eventName: string;
  onClose: () => void;
  isFullscreen?: boolean;
}

export function EventDetailPanel({
  websiteId,
  eventName,
  onClose,
  isFullscreen,
}: EventDetailPanelProps) {
  const { data: propsData, isLoading: propsLoading } = useEventProperties(
    websiteId,
    eventName
  );
  const { data: tsData, isLoading: tsLoading } = useEventTimeseries(
    websiteId,
    eventName
  );

  const result = propsData?.data;
  // Sparkline expects { date, value }[] — map pageviews field to value.
  const series = (tsData?.data?.series ?? []).map((p) => ({
    date: p.date,
    value: p.pageviews,
  }));
  const sampled =
    result !== undefined && result.sample_size < result.total_occurrences;

  return (
    <div
      className={
        isFullscreen
          ? 'fixed inset-0 z-50 bg-canvas flex flex-col overflow-hidden'
          : 'w-[360px] shrink-0 border border-line rounded-lg bg-surface-1 flex flex-col max-h-[80vh] overflow-hidden'
      }
    >
      {/* Header */}
      <div className="px-4 py-3 border-b border-line flex items-center gap-2 shrink-0">
        <span className="flex-1 text-sm font-medium text-ink truncate">
          {eventName}
        </span>
        {result && (
          <span className="text-xs text-ink-3 font-mono tabular-nums">
            {result.total_occurrences.toLocaleString()} occurrences
          </span>
        )}
        <button
          onClick={onClose}
          className="ml-1 p-1 text-ink-3 hover:text-ink rounded-md hover:bg-surface-2 transition-colors"
          aria-label="Close event detail"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Mini timeseries chart */}
      <div className="px-4 py-3 border-b border-line shrink-0">
        <h4 className="text-xs font-semibold text-ink-3 uppercase tracking-wider mb-2">
          Occurrences
        </h4>
        {tsLoading ? (
          <div className="h-8 bg-surface-2 rounded animate-pulse" />
        ) : (
          <Sparkline data={series} color="var(--spark)" />
        )}
      </div>

      {/* Property breakdown */}
      <div className="flex-1 overflow-y-auto">
        <div className="px-4 py-3 border-b border-line flex items-center justify-between">
          <h4 className="text-xs font-semibold text-ink-3 uppercase tracking-wider">
            Properties
          </h4>
          {sampled && result && (
            <span className="text-xs text-ink-3">
              Sampled {result.sample_size.toLocaleString()} of{' '}
              {result.total_occurrences.toLocaleString()}
            </span>
          )}
        </div>

        {propsLoading ? (
          <div className="p-4 space-y-2">
            {Array.from({ length: 6 }).map((_, i) => (
              <div key={i} className="h-6 bg-surface-2 rounded animate-pulse" />
            ))}
          </div>
        ) : result && result.properties.length > 0 ? (
          <EventPropertyTable properties={result.properties} />
        ) : (
          <div className="px-6 py-8 text-center">
            <p className="text-sm text-ink-3">No properties found for this event.</p>
            <p className="text-xs text-ink-3 mt-2">
              Pass a data object when tracking:{' '}
              <code className="font-mono bg-surface-2 px-1 rounded">
                track(&apos;{eventName}&apos;, {`{ key: 'value' }`})
              </code>
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
