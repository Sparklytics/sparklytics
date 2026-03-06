'use client';

import { useEffect, useState } from 'react';
import { Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  usePlanLimits,
  useTenantLimits,
  useTenantUsage,
  useUpdatePlanLimit,
  useUpdateTenantLimits,
} from '@/hooks/useAdminLimits';

export function AdminLimitsPanel() {
  const planLimits = usePlanLimits(true);
  const updatePlanLimit = useUpdatePlanLimit();
  const [tenantInputId, setTenantInputId] = useState('');
  const [tenantId, setTenantId] = useState('');
  const [month, setMonth] = useState('');
  const tenantLimits = useTenantLimits(tenantId, !!tenantId);
  const tenantUsage = useTenantUsage(tenantId, month || undefined, !!tenantId);
  const updateTenantLimits = useUpdateTenantLimits(tenantId);

  const [planEdits, setPlanEdits] = useState<Record<string, { peak: string; monthly: string }>>({});
  const [tenantPeak, setTenantPeak] = useState('');
  const [tenantMonthly, setTenantMonthly] = useState('');

  useEffect(() => {
    const override = tenantLimits.data?.data.override;
    setTenantPeak(
      override?.peak_events_per_sec !== null && override?.peak_events_per_sec !== undefined
        ? String(override.peak_events_per_sec)
        : '',
    );
    setTenantMonthly(
      override?.monthly_event_limit !== null && override?.monthly_event_limit !== undefined
        ? String(override.monthly_event_limit)
        : '',
    );
  }, [tenantId, tenantLimits.data]);

  function parsePositiveInt(raw: string): number | null {
    if (!raw.trim()) return null;
    const parsed = Number(raw);
    if (!Number.isInteger(parsed) || parsed <= 0) return null;
    return parsed;
  }

  const plans = planLimits.data?.data ?? [];

  const planRows = plans.map((plan) => {
    const edit = planEdits[plan.plan] ?? {
      peak: String(plan.peak_events_per_sec),
      monthly: String(plan.monthly_event_limit),
    };
    return { plan, edit };
  });

  const tenantPeakParsed = parsePositiveInt(tenantPeak);
  const tenantMonthlyParsed = parsePositiveInt(tenantMonthly);
  const hasTenantOverrideInput = tenantPeak.trim().length > 0 || tenantMonthly.trim().length > 0;
  const tenantOverrideInputValid =
    (tenantPeak.trim().length === 0 || tenantPeakParsed !== null) &&
    (tenantMonthly.trim().length === 0 || tenantMonthlyParsed !== null);

  if (planLimits.isLoading) {
    return (
      <section className="bg-surface-1 border border-line rounded-lg p-6">
        <div className="flex items-center gap-2 text-sm text-ink-3">
          <Loader2 className="w-4 h-4 animate-spin" />
          Loading admin limits…
        </div>
      </section>
    );
  }

  if (planLimits.isError) {
    return (
      <section className="bg-surface-1 border border-line rounded-lg p-6">
        <p className="text-sm text-down">Admin limits are unavailable for this user or environment.</p>
      </section>
    );
  }

  return (
    <section className="bg-surface-1 border border-line rounded-lg p-6 space-y-6">
      <div>
        <h2 className="text-sm font-semibold text-ink">Cloud admin ingestion limits</h2>
        <p className="text-xs text-ink-3 mt-1">
          Manage plan defaults and per-tenant overrides for peak ingest and monthly event quotas.
        </p>
      </div>

      <div className="space-y-3">
        <h3 className="text-xs font-semibold text-ink uppercase tracking-[0.08em]">Plan defaults</h3>
        {planRows.map(({ plan, edit }) => (
          <div key={plan.plan} className="border border-line rounded-md p-3 space-y-2">
            <p className="text-sm font-medium text-ink">{plan.plan}</p>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <label className="block">
                <span className="text-xs text-ink-2 mb-1 block">Peak events/sec</span>
                <input
                  type="number"
                  min={1}
                  value={edit.peak}
                  onChange={(e) =>
                    setPlanEdits((prev) => ({
                      ...prev,
                      [plan.plan]: { ...edit, peak: e.target.value },
                    }))
                  }
                  className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink"
                />
              </label>
              <label className="block">
                <span className="text-xs text-ink-2 mb-1 block">Monthly event limit</span>
                <input
                  type="number"
                  min={1}
                  value={edit.monthly}
                  onChange={(e) =>
                    setPlanEdits((prev) => ({
                      ...prev,
                      [plan.plan]: { ...edit, monthly: e.target.value },
                    }))
                  }
                  className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink"
                />
              </label>
            </div>
            <Button
              size="sm"
              className="text-xs"
              disabled={
                updatePlanLimit.isPending ||
                !Number.isInteger(Number(edit.peak)) ||
                Number(edit.peak) <= 0 ||
                !Number.isInteger(Number(edit.monthly)) ||
                Number(edit.monthly) <= 0
              }
              onClick={() =>
                updatePlanLimit.mutate({
                  plan: plan.plan,
                  peak_events_per_sec: Number(edit.peak),
                  monthly_event_limit: Number(edit.monthly),
                })
              }
            >
              {updatePlanLimit.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
              Save {plan.plan}
            </Button>
          </div>
        ))}
      </div>

      <div className="border-t border-line pt-4 space-y-3">
        <h3 className="text-xs font-semibold text-ink uppercase tracking-[0.08em]">Tenant override</h3>
        <label className="block">
          <span className="text-xs text-ink-2 mb-1 block">Tenant ID</span>
          <div className="flex gap-2">
            <input
              value={tenantInputId}
              onChange={(e) => setTenantInputId(e.target.value)}
              placeholder="org_..."
              className="flex-1 bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink"
            />
            <Button
              size="sm"
              variant="outline"
              className="text-xs"
              disabled={!tenantInputId.trim()}
              onClick={() => setTenantId(tenantInputId.trim())}
            >
              Load tenant
            </Button>
          </div>
        </label>
        <label className="block">
          <span className="text-xs text-ink-2 mb-1 block">Month (optional, YYYY-MM)</span>
          <input
            value={month}
            onChange={(e) => setMonth(e.target.value)}
            placeholder="2026-03"
            className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink"
          />
        </label>

        {tenantLimits.data && (
          <div className="text-xs text-ink-2 space-y-1">
            <p>Effective plan: <span className="text-ink font-medium">{tenantLimits.data.data.effective.plan}</span></p>
            <p>Effective peak: <span className="font-mono tabular-nums text-ink">{tenantLimits.data.data.effective.peak_events_per_sec}</span> events/sec</p>
            <p>Effective monthly: <span className="font-mono tabular-nums text-ink">{tenantLimits.data.data.effective.monthly_event_limit}</span></p>
          </div>
        )}

        {tenantUsage.data && (
          <div className="text-xs text-ink-2 space-y-1">
            <p>
              Usage {tenantUsage.data.data.month}:{' '}
              <span className="font-mono tabular-nums text-ink">
                {tenantUsage.data.data.event_count}/{tenantUsage.data.data.event_limit}
              </span>{' '}
              ({tenantUsage.data.data.percent_used.toFixed(2)}%)
            </p>
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <label className="block">
            <span className="text-xs text-ink-2 mb-1 block">Override peak events/sec (blank = unchanged)</span>
            <input
              type="number"
              min={1}
              value={tenantPeak}
              onChange={(e) => setTenantPeak(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink"
            />
          </label>
          <label className="block">
            <span className="text-xs text-ink-2 mb-1 block">Override monthly limit (blank = unchanged)</span>
            <input
              type="number"
              min={1}
              value={tenantMonthly}
              onChange={(e) => setTenantMonthly(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink"
            />
          </label>
        </div>

        <div className="flex gap-2">
          <Button
            size="sm"
            className="text-xs"
            disabled={
              !tenantId ||
              updateTenantLimits.isPending ||
              !hasTenantOverrideInput ||
              !tenantOverrideInputValid
            }
            onClick={() => {
              const payload: {
                peak_events_per_sec?: number | null;
                monthly_event_limit?: number | null;
              } = {};
              if (tenantPeak.trim()) payload.peak_events_per_sec = tenantPeakParsed;
              if (tenantMonthly.trim()) payload.monthly_event_limit = tenantMonthlyParsed;
              updateTenantLimits.mutate(payload);
            }}
          >
            {updateTenantLimits.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            Apply override
          </Button>
          <Button
            size="sm"
            variant="outline"
            className="text-xs"
            disabled={!tenantId || updateTenantLimits.isPending}
            onClick={() => updateTenantLimits.mutate({ clear: true })}
          >
            Clear override
          </Button>
        </div>
      </div>
    </section>
  );
}
