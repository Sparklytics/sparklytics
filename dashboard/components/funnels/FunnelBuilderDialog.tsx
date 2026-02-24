'use client';

import { useState, useEffect } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Plus } from 'lucide-react';
import { FunnelStepRow } from './FunnelStepRow';
import { useCreateFunnel, useUpdateFunnel } from '@/hooks/useFunnels';
import { useTopPages } from '@/hooks/useTopPages';
import type { Funnel, CreateFunnelStepPayload } from '@/lib/api';

const inputClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-3 focus:outline-none focus:ring-1 focus:ring-spark focus:border-spark';

const labelClass = 'block text-xs font-medium text-ink-3 mb-1';

interface FunnelBuilderDialogProps {
  websiteId: string;
  open: boolean;
  onClose: () => void;
  editingFunnel?: Funnel | null;
}

// Each step carries a stable synthetic _id so React can key rows without
// using the array index (which breaks focus/state when rows are removed).
type StepWithId = CreateFunnelStepPayload & { _id: string };

function defaultStep(): StepWithId {
  return {
    _id: crypto.randomUUID(),
    step_type: 'page_view',
    match_value: '',
    match_operator: 'equals',
    label: '',
  };
}

export function FunnelBuilderDialog({ websiteId, open, onClose, editingFunnel }: FunnelBuilderDialogProps) {
  const isEditing = !!editingFunnel;

  const [name, setName] = useState('');
  const [steps, setSteps] = useState<StepWithId[]>([defaultStep(), defaultStep()]);
  const [apiError, setApiError] = useState<string | null>(null);

  const createFunnel = useCreateFunnel(websiteId);
  const updateFunnel = useUpdateFunnel(websiteId);
  const pageSuggestions = useTopPages(websiteId);

  useEffect(() => {
    if (open) {
      setName(editingFunnel?.name ?? '');
      setSteps(
        editingFunnel?.steps.map((s) => ({
          _id: crypto.randomUUID(),
          step_type: s.step_type,
          match_value: s.match_value,
          match_operator: s.match_operator,
          label: s.label,
        })) ?? [defaultStep(), defaultStep()]
      );
      setApiError(null);
    }
  }, [open, editingFunnel]);

  const isPending = createFunnel.isPending || updateFunnel.isPending;

  function addStep() {
    if (steps.length >= 8) return;
    setSteps((prev) => [...prev, defaultStep()]);
  }

  function removeStep(idx: number) {
    if (steps.length <= 2) return;
    setSteps((prev) => prev.filter((_, i) => i !== idx));
  }

  function updateStep(idx: number, patch: Partial<CreateFunnelStepPayload>) {
    setSteps((prev) => prev.map((s, i) => (i === idx ? { ...s, ...patch } : s)));
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setApiError(null);

    if (!name.trim()) {
      setApiError('Funnel name is required.');
      return;
    }
    for (let i = 0; i < steps.length; i++) {
      if (!steps[i].match_value.trim()) {
        setApiError(`Step ${i + 1}: match value is required.`);
        return;
      }
    }

    // Strip internal _id before sending to API
    const stepsPayload = steps.map(({ _id: _, ...s }) => ({
      ...s,
      label: s.label?.trim() || s.match_value,
    }));

    if (isEditing && editingFunnel) {
      updateFunnel.mutate(
        { funnelId: editingFunnel.id, payload: { name: name.trim(), steps: stepsPayload } },
        {
          onSuccess: () => onClose(),
          onError: (err) => setApiError(err.message),
        }
      );
    } else {
      createFunnel.mutate(
        { name: name.trim(), steps: stepsPayload },
        {
          onSuccess: () => onClose(),
          onError: (err) => setApiError(err.message),
        }
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-lg max-w-xl">
        <DialogHeader>
          <DialogTitle className="text-base font-semibold text-ink">
            {isEditing ? 'Edit funnel' : 'New funnel'}
          </DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Name */}
          <div>
            <label htmlFor="funnel-name" className={labelClass}>Name</label>
            <input
              id="funnel-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className={inputClass}
              placeholder="e.g. Checkout Funnel"
              maxLength={100}
              required
            />
          </div>

          {/* Steps */}
          <fieldset className="border-0 p-0 m-0">
            <legend className={`${labelClass} w-full`}>Steps</legend>
            <p className="text-xs text-ink-4 mb-2">
              Steps must occur in order within a single session.
            </p>
            <div className="space-y-2">
              {steps.map((step, idx) => (
                <FunnelStepRow
                  key={step._id}
                  index={idx}
                  step={step}
                  canDelete={steps.length > 2}
                  onChange={(patch) => updateStep(idx, patch)}
                  onDelete={() => removeStep(idx)}
                  suggestions={pageSuggestions}
                />
              ))}
            </div>
            {steps.length < 8 && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={addStep}
                className="mt-2 px-0 text-xs text-spark hover:text-spark/80 hover:bg-transparent gap-1"
              >
                <Plus className="w-4 h-4" />
                Add Step
              </Button>
            )}
          </fieldset>

          {apiError && (
            <p className="text-xs text-red-400 border border-red-400/20 bg-red-400/5 rounded-lg px-3 py-2">
              {apiError}
            </p>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onClose}
              className="text-xs"
            >
              Cancel
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={isPending}
              className="text-xs"
            >
              {isPending ? 'Saving…' : isEditing ? 'Save changes' : 'Create funnel'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
