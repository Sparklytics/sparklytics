'use client';

import { useState } from 'react';
import { AnchorType, JourneyDirection } from '@/lib/api';
import { useJourney } from '@/hooks/useJourney';
import { useFilters } from '@/hooks/useFilters';
import { JourneyBranchList } from './JourneyBranchList';
import { JourneyControls } from './JourneyControls';

interface JourneyPageProps {
  websiteId: string;
}

export function JourneyPage({ websiteId }: JourneyPageProps) {
  const { dateRange } = useFilters();

  const [anchorType, setAnchorType] = useState<AnchorType>('page');
  const [anchorValue, setAnchorValue] = useState('');
  const [direction, setDirection] = useState<JourneyDirection>('next');
  const [maxDepth, setMaxDepth] = useState(3);

  const [queryParams, setQueryParams] = useState({
    anchor_type: 'page' as AnchorType,
    anchor_value: '',
    direction: 'next' as JourneyDirection,
    max_depth: 3,
  });

  const { data, isLoading, error } = useJourney(websiteId, queryParams);

  const hasQuery = queryParams.anchor_value.trim().length > 0;

  function runSearch() {
    const value = anchorValue.trim();
    if (!value) return;
    setQueryParams({
      anchor_type: anchorType,
      anchor_value: value,
      direction,
      max_depth: maxDepth,
    });
  }

  function handleDirectionChange(nextDirection: JourneyDirection) {
    setDirection(nextDirection);
    if (hasQuery) {
      setQueryParams((prev) => ({ ...prev, direction: nextDirection }));
    }
  }

  function handleDepthChange(nextDepth: number) {
    setMaxDepth(nextDepth);
    if (hasQuery) {
      setQueryParams((prev) => ({ ...prev, max_depth: nextDepth }));
    }
  }

  function handlePickNode(value: string) {
    const inferredType: AnchorType = value.startsWith('/') ? 'page' : 'event';
    setAnchorType(inferredType);
    setAnchorValue(value);
    setQueryParams((prev) => ({
      ...prev,
      anchor_type: inferredType,
      anchor_value: value,
    }));
  }

  const result = data?.data;

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">Journey</h2>
        <p className="text-xs text-ink-3 mt-1">
          Explore what users did before or after a page/event anchor.
        </p>
      </div>

      <JourneyControls
        anchorType={anchorType}
        anchorValue={anchorValue}
        direction={direction}
        maxDepth={maxDepth}
        onAnchorTypeChange={setAnchorType}
        onAnchorValueChange={setAnchorValue}
        onDirectionChange={handleDirectionChange}
        onMaxDepthChange={handleDepthChange}
        onSearch={runSearch}
      />

      {!hasQuery ? (
        <div className="border border-line rounded-lg bg-surface-1 px-4 py-10 text-center">
          <p className="text-sm font-medium text-ink">No anchor selected</p>
          <p className="text-xs text-ink-3 mt-1">
            Enter a page URL or event name to explore navigation branches.
          </p>
        </div>
      ) : isLoading && !result ? (
        <div className="space-y-2">
          {Array.from({ length: 4 }).map((_, idx) => (
            <div key={idx} className="h-16 border border-line rounded-md bg-surface-1 animate-pulse" />
          ))}
        </div>
      ) : error ? (
        <div className="border border-line rounded-lg bg-surface-1 px-4 py-6">
          <p className="text-sm text-red-400">{(error as Error).message}</p>
        </div>
      ) : result ? (
        <div className="space-y-3">
          <div className="border border-line rounded-lg bg-surface-1 px-4 py-3 flex flex-wrap items-center gap-2 justify-between">
            <p className="text-xs text-ink-2">
              <span className="font-mono tabular-nums text-ink">
                {result.total_anchor_sessions.toLocaleString()}
              </span>{' '}
              sessions matched{' '}
              <span className="font-mono text-ink">{result.anchor.value}</span>
            </p>
            <p className="text-xs text-ink-3 font-mono tabular-nums">
              {dateRange.start_date} → {dateRange.end_date}
            </p>
          </div>

          {result.total_anchor_sessions === 0 ? (
            <div className="border border-line rounded-lg bg-surface-1 px-4 py-10 text-center">
              <p className="text-sm font-medium text-ink">No matching sessions</p>
              <p className="text-xs text-ink-3 mt-1">
                Try a different anchor or date range.
              </p>
            </div>
          ) : (
            <JourneyBranchList
              branches={result.branches}
              direction={result.direction}
              onPickNode={handlePickNode}
            />
          )}
        </div>
      ) : null}
    </div>
  );
}
