'use client';

import { X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { CreateFunnelStepPayload, MatchOperator, StepType } from '@/lib/api';

const inputClass =
  'flex-1 px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-3 focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

const selectClass =
  'px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

interface FunnelStepRowProps {
  index: number;
  step: CreateFunnelStepPayload;
  canDelete: boolean;
  onChange: (patch: Partial<CreateFunnelStepPayload>) => void;
  onDelete: () => void;
  suggestions?: string[];
}

export function FunnelStepRow({ index, step, canDelete, onChange, onDelete, suggestions }: FunnelStepRowProps) {
  const datalistId = `page-suggestions-${index}`;
  const showSuggestions = step.step_type === 'page_view' && !!suggestions?.length;

  return (
    <div className="flex items-center gap-2">
      <span className="text-ink-4 font-mono text-xs w-4 shrink-0 text-center">{index + 1}</span>

      <select
        value={step.step_type}
        onChange={(e) => onChange({ step_type: e.target.value as StepType })}
        className={`${selectClass} w-32 shrink-0`}
        aria-label="Step type"
      >
        <option value="page_view">Page View</option>
        <option value="event">Event</option>
      </select>

      {showSuggestions && (
        <datalist id={datalistId}>
          {suggestions!.map((path) => (
            <option key={path} value={path} />
          ))}
        </datalist>
      )}

      <input
        type="text"
        value={step.match_value}
        onChange={(e) => onChange({ match_value: e.target.value })}
        placeholder={step.step_type === 'page_view' ? '/path' : 'event_name'}
        className={inputClass}
        maxLength={500}
        aria-label="Match value"
        list={showSuggestions ? datalistId : undefined}
      />

      <select
        value={step.match_operator ?? 'equals'}
        onChange={(e) => onChange({ match_operator: e.target.value as MatchOperator })}
        className={`${selectClass} w-28 shrink-0`}
        aria-label="Match operator"
      >
        <option value="equals">Equals</option>
        <option value="contains">Contains</option>
      </select>

      <Button
        type="button"
        variant="ghost"
        size="icon"
        onClick={onDelete}
        disabled={!canDelete}
        className="shrink-0 text-ink-4 hover:text-down hover:bg-down/10 disabled:opacity-30"
        aria-label="Remove step"
      >
        <X className="w-4 h-4" />
      </Button>
    </div>
  );
}
