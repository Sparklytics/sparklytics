'use client';

import { AttributionModel } from '@/lib/api';
import { cn } from '@/lib/utils';

interface AttributionModelToggleProps {
  model: AttributionModel;
  onChange: (model: AttributionModel) => void;
}

const OPTIONS: Array<{ value: AttributionModel; label: string; hint: string }> = [
  {
    value: 'last_touch',
    label: 'Last touch',
    hint: 'Credit the latest channel before conversion.',
  },
  {
    value: 'first_touch',
    label: 'First touch',
    hint: 'Credit the first channel in the conversion session.',
  },
];

export function AttributionModelToggle({ model, onChange }: AttributionModelToggleProps) {
  return (
    <div className="border border-line rounded-lg bg-surface-1 p-3">
      <p className="text-xs font-medium text-ink-2 mb-2">Attribution model</p>
      <div className="flex flex-wrap gap-2">
        {OPTIONS.map((option) => {
          const isActive = model === option.value;
          return (
            <button
              key={option.value}
              type="button"
              onClick={() => onChange(option.value)}
              className={cn(
                'px-3 py-2 rounded-md border text-left transition-colors min-w-[180px]',
                isActive
                  ? 'border-spark bg-spark/10 text-ink'
                  : 'border-line bg-canvas text-ink-3 hover:text-ink-2 hover:border-line/80'
              )}
            >
              <p className="text-xs font-medium">{option.label}</p>
              <p className="text-[11px] text-ink-4 mt-0.5">{option.hint}</p>
            </button>
          );
        })}
      </div>
    </div>
  );
}
