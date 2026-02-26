'use client';

import { useEffect, useMemo, useState } from 'react';
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { Loader2, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  BotListEntry,
  BotMatchType,
  BotPolicyMode,
  BotPolicyAuditRecord,
} from '@/lib/api';
import { formatNumber } from '@/lib/utils';
import {
  useBotAllowlist,
  useBotAudit,
  useBotBlocklist,
  useBotPolicy,
  useBotRecomputeStatus,
  useBotReport,
  useBotSummary,
  useCreateBotAllowlist,
  useCreateBotBlocklist,
  useDeleteBotAllowlist,
  useDeleteBotBlocklist,
  useStartBotRecompute,
  useUpdateBotPolicy,
} from '@/hooks/useBots';

function fmtDate(date: Date): string {
  return date.toISOString().slice(0, 10);
}

function dateDaysAgo(days: number): string {
  const now = new Date();
  now.setDate(now.getDate() - days);
  return fmtDate(now);
}

function modeDefaultThreshold(mode: BotPolicyMode): number {
  switch (mode) {
    case 'strict':
      return 60;
    case 'balanced':
      return 70;
    case 'off':
      return 70;
    default:
      return 70;
  }
}

function AuditPayload({ record }: { record: BotPolicyAuditRecord }) {
  const compact = JSON.stringify(record.payload);
  return (
    <p className="text-[11px] text-ink-3 font-mono tabular-nums break-all">
      {compact}
    </p>
  );
}

function ListTable({
  title,
  rows,
  onDelete,
  deleting,
}: {
  title: string;
  rows: BotListEntry[];
  onDelete: (id: string) => void;
  deleting: boolean;
}) {
  return (
    <section className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
      <h3 className="text-sm font-semibold text-ink">{title}</h3>
      {rows.length === 0 ? (
        <p className="text-xs text-ink-3">No entries yet.</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-line text-ink-3">
                <th className="text-left py-2 pr-2">Match</th>
                <th className="text-left py-2 pr-2">Value</th>
                <th className="text-left py-2 pr-2">Note</th>
                <th className="text-right py-2">Action</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.id} className="border-b border-line/50">
                  <td className="py-2 pr-2 font-mono tabular-nums">{row.match_type}</td>
                  <td className="py-2 pr-2 font-mono tabular-nums">{row.match_value}</td>
                  <td className="py-2 pr-2 text-ink-2">{row.note ?? '-'}</td>
                  <td className="py-2 text-right">
                    <Button
                      variant="outline"
                      size="sm"
                      className="text-xs"
                      onClick={() => onDelete(row.id)}
                      disabled={deleting}
                    >
                      <Trash2 className="w-3 h-3 mr-1" />
                      Remove
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

export function BotsSettingsPage({ websiteId }: { websiteId: string }) {
  const [startDate, setStartDate] = useState(() => dateDaysAgo(6));
  const [endDate, setEndDate] = useState(() => fmtDate(new Date()));

  const [mode, setMode] = useState<BotPolicyMode>('balanced');
  const [threshold, setThreshold] = useState(70);

  const [allowMatchType, setAllowMatchType] = useState<BotMatchType>('ua_contains');
  const [allowValue, setAllowValue] = useState('');
  const [allowNote, setAllowNote] = useState('');

  const [blockMatchType, setBlockMatchType] = useState<BotMatchType>('ua_contains');
  const [blockValue, setBlockValue] = useState('');
  const [blockNote, setBlockNote] = useState('');

  const [recomputeDays, setRecomputeDays] = useState(7);
  const [recomputeJobId, setRecomputeJobId] = useState<string | null>(null);

  const policyQuery = useBotPolicy(websiteId);
  const summaryQuery = useBotSummary(websiteId, { start_date: startDate, end_date: endDate });
  const reportQuery = useBotReport(websiteId, {
    start_date: startDate,
    end_date: endDate,
    granularity: 'day',
  });
  const allowlistQuery = useBotAllowlist(websiteId, { limit: 50 });
  const blocklistQuery = useBotBlocklist(websiteId, { limit: 50 });
  const auditQuery = useBotAudit(websiteId, { limit: 20 });

  const updatePolicy = useUpdateBotPolicy(websiteId);
  const createAllow = useCreateBotAllowlist(websiteId);
  const createBlock = useCreateBotBlocklist(websiteId);
  const deleteAllow = useDeleteBotAllowlist(websiteId);
  const deleteBlock = useDeleteBotBlocklist(websiteId);
  const startRecompute = useStartBotRecompute(websiteId);
  const recomputeStatus = useBotRecomputeStatus(websiteId, recomputeJobId);

  useEffect(() => {
    const policy = policyQuery.data?.data;
    if (!policy) return;
    setMode(policy.mode);
    setThreshold(policy.threshold_score);
  }, [policyQuery.data]);

  const report = reportQuery.data?.data;
  const summary = summaryQuery.data?.data;

  const trendData = useMemo(
    () =>
      (report?.timeseries ?? []).map((row) => {
        const date = new Date(row.period_start);
        return {
          date: row.period_start,
          label: `${date.getMonth() + 1}/${date.getDate()}`,
          bot_events: row.bot_events,
          human_events: row.human_events,
        };
      }),
    [report?.timeseries]
  );

  async function handleSavePolicy() {
    const safeThreshold = Math.max(0, Math.min(100, threshold));
    await updatePolicy.mutateAsync({ mode, threshold_score: safeThreshold });
  }

  async function handleAddAllow() {
    if (!allowValue.trim()) return;
    await createAllow.mutateAsync({
      match_type: allowMatchType,
      match_value: allowValue.trim(),
      note: allowNote.trim() || undefined,
    });
    setAllowValue('');
    setAllowNote('');
  }

  async function handleAddBlock() {
    if (!blockValue.trim()) return;
    await createBlock.mutateAsync({
      match_type: blockMatchType,
      match_value: blockValue.trim(),
      note: blockNote.trim() || undefined,
    });
    setBlockValue('');
    setBlockNote('');
  }

  async function handleStartRecompute() {
    const safeDays = Math.max(1, Math.min(30, recomputeDays));
    const now = new Date();
    const end = fmtDate(now);
    const start = dateDaysAgo(safeDays - 1);
    const result = await startRecompute.mutateAsync({
      start_date: start,
      end_date: end,
    });
    setRecomputeJobId(result.job_id);
  }

  const loadingSummary = summaryQuery.isLoading || reportQuery.isLoading;

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink">Bots</h2>
        <p className="text-xs text-ink-3 mt-0.5">
          Configure bot policy, overrides, visibility, and recompute for this website.
        </p>
      </div>

      <section className="border border-line rounded-lg bg-surface-1 p-4 space-y-4">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-sm font-semibold text-ink">Policy</h3>
          <Button
            size="sm"
            className="text-xs"
            onClick={handleSavePolicy}
            disabled={updatePolicy.isPending || policyQuery.isLoading}
          >
            {updatePolicy.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            Save policy
          </Button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          {(['strict', 'balanced', 'off'] as BotPolicyMode[]).map((candidate) => (
            <button
              key={candidate}
              type="button"
              onClick={() => {
                setMode(candidate);
                if (candidate !== 'off') {
                  setThreshold(modeDefaultThreshold(candidate));
                }
              }}
              className={`border rounded-md px-3 py-2 text-left transition-colors ${
                mode === candidate
                  ? 'border-spark text-ink bg-canvas'
                  : 'border-line text-ink-3 hover:text-ink-2'
              }`}
            >
              <p className="text-xs font-semibold uppercase tracking-[0.08em]">{candidate}</p>
              <p className="text-[11px] mt-1 text-ink-3">
                {candidate === 'strict' && 'Lower threshold, more aggressive bot filtering.'}
                {candidate === 'balanced' && 'Default detection threshold for mixed traffic.'}
                {candidate === 'off' && 'Disable bot filtering in default analytics views.'}
              </p>
            </button>
          ))}
        </div>

        <div className="max-w-[220px]">
          <label className="block text-xs text-ink-2 mb-1">Threshold score (0-100)</label>
          <Input
            type="number"
            min={0}
            max={100}
            value={threshold}
            disabled={mode === 'off'}
            onChange={(event) => setThreshold(Number(event.target.value || 0))}
            className="h-8 text-xs"
          />
        </div>
      </section>

      <section className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-sm font-semibold text-ink">Report Window</h3>
          <div className="flex items-center gap-2">
            <Input
              type="date"
              value={startDate}
              onChange={(event) => setStartDate(event.target.value)}
              className="h-8 text-xs"
            />
            <Input
              type="date"
              value={endDate}
              onChange={(event) => setEndDate(event.target.value)}
              className="h-8 text-xs"
            />
          </div>
        </div>

        {loadingSummary ? (
          <div className="py-8 flex items-center justify-center">
            <Loader2 className="w-5 h-5 animate-spin text-ink-3" />
          </div>
        ) : (
          <>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <div className="border border-line rounded-md p-3 bg-canvas">
                <p className="text-[11px] text-ink-3 uppercase tracking-[0.08em]">Bot Events</p>
                <p className="mt-1 text-lg text-ink font-mono tabular-nums">
                  {formatNumber(summary?.bot_events ?? report?.split.bot_events ?? 0)}
                </p>
              </div>
              <div className="border border-line rounded-md p-3 bg-canvas">
                <p className="text-[11px] text-ink-3 uppercase tracking-[0.08em]">Human Events</p>
                <p className="mt-1 text-lg text-ink font-mono tabular-nums">
                  {formatNumber(summary?.human_events ?? report?.split.human_events ?? 0)}
                </p>
              </div>
              <div className="border border-line rounded-md p-3 bg-canvas">
                <p className="text-[11px] text-ink-3 uppercase tracking-[0.08em]">Bot Rate</p>
                <p className="mt-1 text-lg text-ink font-mono tabular-nums">
                  {(((summary?.bot_rate ?? report?.split.bot_rate ?? 0) * 100).toFixed(2))}%
                </p>
              </div>
            </div>

            <div className="h-[220px] border border-line rounded-md p-2 bg-canvas">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={trendData} margin={{ top: 4, right: 8, bottom: 0, left: -16 }}>
                  <CartesianGrid stroke="var(--line)" strokeDasharray="3 3" vertical={false} />
                  <XAxis dataKey="label" tick={{ fill: 'var(--ink-3)', fontSize: 11 }} tickLine={false} axisLine={false} />
                  <YAxis tick={{ fill: 'var(--ink-3)', fontSize: 11 }} tickLine={false} axisLine={false} />
                  <Tooltip />
                  <Line type="monotone" dataKey="human_events" stroke="var(--spark)" strokeWidth={2} dot={false} isAnimationActive={false} />
                  <Line type="monotone" dataKey="bot_events" stroke="var(--neutral)" strokeWidth={2} dot={false} isAnimationActive={false} />
                </LineChart>
              </ResponsiveContainer>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div className="border border-line rounded-md p-3 bg-canvas">
                <h4 className="text-xs font-semibold text-ink mb-2">Top Bot Reasons</h4>
                {(report?.top_reasons ?? summary?.top_reasons ?? []).length === 0 ? (
                  <p className="text-xs text-ink-3">No bot traffic in selected range.</p>
                ) : (
                  <div className="space-y-1.5">
                    {(report?.top_reasons ?? summary?.top_reasons ?? []).map((row) => (
                      <div key={row.code} className="flex items-center justify-between text-xs">
                        <span className="font-mono text-ink-2">{row.code}</span>
                        <span className="font-mono tabular-nums text-ink">{formatNumber(row.count)}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <div className="border border-line rounded-md p-3 bg-canvas">
                <h4 className="text-xs font-semibold text-ink mb-2">Top User Agents</h4>
                {(report?.top_user_agents ?? []).length === 0 ? (
                  <p className="text-xs text-ink-3">No bot user-agent records yet.</p>
                ) : (
                  <div className="space-y-1.5">
                    {(report?.top_user_agents ?? []).map((row) => (
                      <div key={row.value} className="flex items-center justify-between text-xs gap-2">
                        <span className="font-mono text-ink-2 truncate">{row.value}</span>
                        <span className="font-mono tabular-nums text-ink shrink-0">{formatNumber(row.count)}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </>
        )}
      </section>

      <section className="grid grid-cols-1 xl:grid-cols-2 gap-4">
        <div className="space-y-3">
          <div className="border border-line rounded-lg bg-surface-1 p-4 space-y-2">
            <h3 className="text-sm font-semibold text-ink">Add Allowlist Entry</h3>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
              <select
                value={allowMatchType}
                onChange={(event) => setAllowMatchType(event.target.value as BotMatchType)}
                className="h-8 bg-canvas border border-line rounded-md px-2 text-xs text-ink"
              >
                <option value="ua_contains">ua_contains</option>
                <option value="ip_exact">ip_exact</option>
                <option value="ip_cidr">ip_cidr</option>
              </select>
              <Input
                value={allowValue}
                onChange={(event) => setAllowValue(event.target.value)}
                placeholder="match value"
                className="h-8 text-xs"
              />
              <Input
                value={allowNote}
                onChange={(event) => setAllowNote(event.target.value)}
                placeholder="note (optional)"
                className="h-8 text-xs"
              />
            </div>
            <Button
              size="sm"
              className="text-xs"
              onClick={handleAddAllow}
              disabled={createAllow.isPending || !allowValue.trim()}
            >
              {createAllow.isPending ? <Loader2 className="w-3 h-3 mr-1 animate-spin" /> : <Plus className="w-3 h-3 mr-1" />}
              Add to allowlist
            </Button>
          </div>
          <ListTable
            title="Allowlist"
            rows={allowlistQuery.data?.data ?? []}
            onDelete={(id) => deleteAllow.mutate(id)}
            deleting={deleteAllow.isPending}
          />
        </div>

        <div className="space-y-3">
          <div className="border border-line rounded-lg bg-surface-1 p-4 space-y-2">
            <h3 className="text-sm font-semibold text-ink">Add Blocklist Entry</h3>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
              <select
                value={blockMatchType}
                onChange={(event) => setBlockMatchType(event.target.value as BotMatchType)}
                className="h-8 bg-canvas border border-line rounded-md px-2 text-xs text-ink"
              >
                <option value="ua_contains">ua_contains</option>
                <option value="ip_exact">ip_exact</option>
                <option value="ip_cidr">ip_cidr</option>
              </select>
              <Input
                value={blockValue}
                onChange={(event) => setBlockValue(event.target.value)}
                placeholder="match value"
                className="h-8 text-xs"
              />
              <Input
                value={blockNote}
                onChange={(event) => setBlockNote(event.target.value)}
                placeholder="note (optional)"
                className="h-8 text-xs"
              />
            </div>
            <Button
              size="sm"
              className="text-xs"
              onClick={handleAddBlock}
              disabled={createBlock.isPending || !blockValue.trim()}
            >
              {createBlock.isPending ? <Loader2 className="w-3 h-3 mr-1 animate-spin" /> : <Plus className="w-3 h-3 mr-1" />}
              Add to blocklist
            </Button>
          </div>
          <ListTable
            title="Blocklist"
            rows={blocklistQuery.data?.data ?? []}
            onDelete={(id) => deleteBlock.mutate(id)}
            deleting={deleteBlock.isPending}
          />
        </div>
      </section>

      <section className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-sm font-semibold text-ink">Recompute</h3>
          <div className="flex items-center gap-2">
            <Input
              type="number"
              min={1}
              max={30}
              value={recomputeDays}
              onChange={(event) => setRecomputeDays(Number(event.target.value || 1))}
              className="h-8 w-[110px] text-xs"
            />
            <Button
              size="sm"
              className="text-xs"
              onClick={handleStartRecompute}
              disabled={startRecompute.isPending}
            >
              {startRecompute.isPending ? (
                <Loader2 className="w-3 h-3 mr-1 animate-spin" />
              ) : (
                <RefreshCw className="w-3 h-3 mr-1" />
              )}
              Recompute last N days
            </Button>
          </div>
        </div>

        {recomputeJobId && (
          <div className="border border-line rounded-md bg-canvas p-3 text-xs space-y-1">
            <p className="text-ink-3">Job: <span className="font-mono tabular-nums text-ink">{recomputeJobId}</span></p>
            <p className="text-ink-3">
              Status:{' '}
              <span className="font-semibold text-ink">
                {recomputeStatus.data?.data.status ?? 'queued'}
              </span>
            </p>
            {recomputeStatus.data?.data.error_message && (
              <p className="text-down">{recomputeStatus.data.data.error_message}</p>
            )}
          </div>
        )}
      </section>

      <section className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
        <h3 className="text-sm font-semibold text-ink">Policy Audit</h3>
        {(auditQuery.data?.data ?? []).length === 0 ? (
          <p className="text-xs text-ink-3">No audit records yet.</p>
        ) : (
          <div className="space-y-2">
            {(auditQuery.data?.data ?? []).map((record) => (
              <div key={record.id} className="border border-line rounded-md p-3 bg-canvas">
                <div className="flex items-center justify-between gap-2">
                  <p className="text-xs font-semibold text-ink">{record.action}</p>
                  <p className="text-[11px] text-ink-3 font-mono tabular-nums">{record.created_at}</p>
                </div>
                <p className="text-[11px] text-ink-3 mb-1">actor: {record.actor}</p>
                <AuditPayload record={record} />
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
