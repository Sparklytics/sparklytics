'use client';

import { AnchorType, JourneyDirection } from '@/lib/api';
import { Button } from '@/components/ui/button';

interface JourneyControlsProps {
  anchorType: AnchorType;
  anchorValue: string;
  direction: JourneyDirection;
  maxDepth: number;
  onAnchorTypeChange: (value: AnchorType) => void;
  onAnchorValueChange: (value: string) => void;
  onDirectionChange: (value: JourneyDirection) => void;
  onMaxDepthChange: (value: number) => void;
  onSearch: () => void;
}

export function JourneyControls({
  anchorType,
  anchorValue,
  direction,
  maxDepth,
  onAnchorTypeChange,
  onAnchorValueChange,
  onDirectionChange,
  onMaxDepthChange,
  onSearch,
}: JourneyControlsProps) {
  return (
    <div className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
      <div className="grid grid-cols-1 md:grid-cols-[140px_1fr_auto] gap-2">
        <select
          value={anchorType}
          onChange={(event) => onAnchorTypeChange(event.target.value as AnchorType)}
          className="h-8 bg-surface-2 border border-line rounded-md px-2 text-xs text-ink"
          aria-label="Anchor type"
        >
          <option value="page">Page URL</option>
          <option value="event">Event name</option>
        </select>

        <input
          value={anchorValue}
          onChange={(event) => onAnchorValueChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') onSearch();
          }}
          placeholder={anchorType === 'page' ? '/pricing' : 'signup_clicked'}
          className="h-8 bg-surface-2 border border-line rounded-md px-2 text-xs text-ink font-mono"
          aria-label="Anchor value"
        />

        <Button
          size="sm"
          onClick={onSearch}
          disabled={anchorValue.trim().length === 0}
          className="text-xs"
        >
          Search
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <div className="flex items-center gap-1">
          <button
            onClick={() => onDirectionChange('previous')}
            className={`px-2 py-1 rounded-sm border text-xs transition-colors ${
              direction === 'previous'
                ? 'border-spark text-ink bg-spark/10'
                : 'border-line text-ink-3 hover:text-ink-2 hover:border-ink-4'
            }`}
            type="button"
          >
            Before
          </button>
          <button
            onClick={() => onDirectionChange('next')}
            className={`px-2 py-1 rounded-sm border text-xs transition-colors ${
              direction === 'next'
                ? 'border-spark text-ink bg-spark/10'
                : 'border-line text-ink-3 hover:text-ink-2 hover:border-ink-4'
            }`}
            type="button"
          >
            After
          </button>
        </div>

        <div className="ml-auto flex items-center gap-2">
          <label htmlFor="journey-depth" className="text-xs text-ink-3">
            Depth
          </label>
          <select
            id="journey-depth"
            value={String(maxDepth)}
            onChange={(event) => onMaxDepthChange(Number(event.target.value))}
            className="h-8 w-16 bg-surface-2 border border-line rounded-md px-2 text-xs text-ink"
          >
            {[1, 2, 3, 4, 5].map((depth) => (
              <option key={depth} value={depth}>
                {depth}
              </option>
            ))}
          </select>
        </div>
      </div>
    </div>
  );
}
