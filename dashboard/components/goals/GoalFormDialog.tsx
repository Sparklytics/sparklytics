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
import { useCreateGoal, useUpdateGoal } from '@/hooks/useGoals';
import type { Goal, GoalType, GoalValueMode, MatchOperator } from '@/lib/api';

interface GoalFormDialogProps {
  websiteId: string;
  open: boolean;
  onClose: () => void;
  editingGoal?: Goal | null;
}

const inputClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-3 focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark disabled:opacity-50 disabled:cursor-not-allowed';

const selectClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark disabled:opacity-50 disabled:cursor-not-allowed';

const labelClass = 'block text-xs font-medium text-ink-3 mb-1';

export function GoalFormDialog({ websiteId, open, onClose, editingGoal }: GoalFormDialogProps) {
  const isEditing = !!editingGoal;

  const [name, setName] = useState('');
  const [goalType, setGoalType] = useState<GoalType>('page_view');
  const [matchValue, setMatchValue] = useState('');
  const [matchOperator, setMatchOperator] = useState<MatchOperator>('equals');
  const [valueMode, setValueMode] = useState<GoalValueMode>('none');
  const [fixedValue, setFixedValue] = useState('');
  const [valuePropertyKey, setValuePropertyKey] = useState('');
  const [currency, setCurrency] = useState('USD');
  const [apiError, setApiError] = useState<string | null>(null);

  const createGoal = useCreateGoal(websiteId);
  const updateGoal = useUpdateGoal(websiteId);

  useEffect(() => {
    if (open) {
      setName(editingGoal?.name ?? '');
      setGoalType(editingGoal?.goal_type ?? 'page_view');
      setMatchValue(editingGoal?.match_value ?? '');
      setMatchOperator(editingGoal?.match_operator ?? 'equals');
      setValueMode(editingGoal?.value_mode ?? 'none');
      setFixedValue(editingGoal?.fixed_value != null ? String(editingGoal.fixed_value) : '');
      setValuePropertyKey(editingGoal?.value_property_key ?? '');
      setCurrency(editingGoal?.currency ?? 'USD');
      setApiError(null);
    }
  }, [open, editingGoal]);

  const isPending = createGoal.isPending || updateGoal.isPending;

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setApiError(null);

    if (isEditing && editingGoal) {
      updateGoal.mutate(
        {
          goalId: editingGoal.id,
          payload: {
            name: name.trim(),
            match_value: matchValue.trim(),
            match_operator: matchOperator,
            value_mode: valueMode,
            fixed_value: valueMode === 'fixed' && fixedValue.trim() !== '' ? Number(fixedValue) : undefined,
            value_property_key: valueMode === 'event_property' ? valuePropertyKey.trim() : undefined,
            currency: currency.trim().toUpperCase(),
          },
        },
        {
          onSuccess: () => onClose(),
          onError: (err) => setApiError(err.message),
        }
      );
    } else {
      createGoal.mutate(
        {
          name: name.trim(),
          goal_type: goalType,
          match_value: matchValue.trim(),
          match_operator: matchOperator,
          value_mode: valueMode,
          fixed_value: valueMode === 'fixed' && fixedValue.trim() !== '' ? Number(fixedValue) : undefined,
          value_property_key: valueMode === 'event_property' ? valuePropertyKey.trim() : undefined,
          currency: currency.trim().toUpperCase(),
        },
        {
          onSuccess: () => onClose(),
          onError: (err) => setApiError(err.message),
        }
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-lg max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base font-semibold text-ink">
            {isEditing ? 'Edit goal' : 'New goal'}
          </DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Name */}
          <div>
            <label className={labelClass}>Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className={inputClass}
              placeholder="e.g. Purchase completion"
              maxLength={100}
              required
            />
          </div>

          {/* Goal Type */}
          <div>
            <label className={labelClass}>
              Type{isEditing && <span className="ml-1 text-ink-3">(cannot be changed)</span>}
            </label>
            <select
              value={goalType}
              onChange={(e) => setGoalType(e.target.value as GoalType)}
              className={selectClass}
              disabled={isEditing}
            >
              <option value="page_view">Page View</option>
              <option value="event">Custom Event</option>
            </select>
          </div>

          {/* Match Value */}
          <div>
            <label className={labelClass}>
              {goalType === 'page_view' ? 'URL' : 'Event name'}
            </label>
            <input
              type="text"
              value={matchValue}
              onChange={(e) => setMatchValue(e.target.value)}
              className={inputClass}
              placeholder={goalType === 'page_view' ? 'e.g. /checkout/confirmation' : 'e.g. purchase'}
              maxLength={500}
              required
            />
          </div>

          {/* Match Operator */}
          <div>
            <label className={labelClass}>Match operator</label>
            <select
              value={matchOperator}
              onChange={(e) => setMatchOperator(e.target.value as MatchOperator)}
              className={selectClass}
            >
              <option value="equals">Equals (exact match)</option>
              <option value="contains">Contains</option>
            </select>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className={labelClass}>Value mode</label>
              <select
                value={valueMode}
                onChange={(e) => setValueMode(e.target.value as GoalValueMode)}
                className={selectClass}
              >
                <option value="none">None</option>
                <option value="fixed">Fixed value</option>
                <option value="event_property">Event property</option>
              </select>
            </div>
            <div>
              <label className={labelClass}>Currency</label>
              <input
                type="text"
                value={currency}
                onChange={(e) => setCurrency(e.target.value.toUpperCase())}
                className={inputClass}
                maxLength={8}
                placeholder="USD"
              />
            </div>
          </div>

          {valueMode === 'fixed' && (
            <div>
              <label className={labelClass}>Fixed value</label>
              <input
                type="number"
                min={0}
                step="0.01"
                value={fixedValue}
                onChange={(e) => setFixedValue(e.target.value)}
                className={inputClass}
                placeholder="e.g. 49.99"
              />
            </div>
          )}

          {valueMode === 'event_property' && (
            <div>
              <label className={labelClass}>Value property key</label>
              <input
                type="text"
                value={valuePropertyKey}
                onChange={(e) => setValuePropertyKey(e.target.value)}
                className={inputClass}
                placeholder="e.g. amount"
              />
            </div>
          )}

          {apiError && (
            <p className="text-xs text-down border border-down/20 bg-down/5 rounded-md px-3 py-2">
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
              {isPending ? 'Saving…' : isEditing ? 'Save changes' : 'Create goal'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
