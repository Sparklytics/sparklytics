'use client';

import { RetentionCohortRow, RetentionGranularity } from '@/lib/api';
import { RetentionTooltip } from './RetentionTooltip';

interface RetentionHeatmapProps {
  rows: RetentionCohortRow[];
  granularity: RetentionGranularity;
  maxPeriods: number;
  endDate: string;
}

function parseDateLike(input: string): Date | null {
  const match = input.match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]) - 1;
  const day = Number(match[3]);
  const date = new Date(Date.UTC(year, month, day));
  if (Number.isNaN(date.getTime())) return null;
  return date;
}

function addPeriods(base: Date, granularity: RetentionGranularity, offset: number): Date {
  const date = new Date(base.getTime());
  if (granularity === 'day') {
    date.setUTCDate(date.getUTCDate() + offset);
  } else if (granularity === 'week') {
    date.setUTCDate(date.getUTCDate() + offset * 7);
  } else {
    date.setUTCMonth(date.getUTCMonth() + offset);
  }
  return date;
}

function isNotElapsed(
  cohortStart: string,
  granularity: RetentionGranularity,
  offset: number,
  endDate: string
): boolean {
  if (offset === 0) return false;
  const cohortDate = parseDateLike(cohortStart);
  const end = parseDateLike(endDate);
  if (!cohortDate || !end) return false;
  const periodStart = addPeriods(cohortDate, granularity, offset);
  return periodStart.getTime() > end.getTime();
}

function columnLabel(granularity: RetentionGranularity, offset: number): string {
  if (granularity === 'day') return `D${offset}`;
  if (granularity === 'week') return `W${offset}`;
  return `M${offset}`;
}

function cellAriaLabel(
  cohortStart: string,
  granularity: RetentionGranularity,
  offset: number,
  retained: number,
  cohortSize: number,
  rate: number,
  notElapsed: boolean
): string {
  const period =
    granularity === 'day' ? `Day ${offset}` : granularity === 'week' ? `Week ${offset}` : `Month ${offset}`;
  if (notElapsed) {
    return `Cohort ${cohortStart}, ${period}, not yet elapsed`;
  }
  return `Cohort ${cohortStart}, ${period}, ${retained} of ${cohortSize} retained, ${Math.round(
    rate * 100
  )} percent`;
}

function rateColor(rate: number): string {
  if (rate <= 0) return 'bg-zinc-800 text-ink-3';
  if (rate <= 0.2) return 'bg-emerald-900/30 text-ink-2';
  if (rate <= 0.4) return 'bg-emerald-800/50 text-ink';
  if (rate <= 0.6) return 'bg-emerald-700/60 text-ink';
  if (rate <= 0.8) return 'bg-emerald-600/70 text-ink';
  return 'bg-emerald-500/80 text-black';
}

export function RetentionHeatmap({
  rows,
  granularity,
  maxPeriods,
  endDate,
}: RetentionHeatmapProps) {
  if (rows.length === 0) {
    return (
      <div className="border border-line rounded-lg bg-surface-1 px-4 py-10 text-center">
        <p className="text-sm font-medium text-ink">No retention data in this range</p>
        <p className="text-xs text-ink-3 mt-1">Try widening the date range.</p>
      </div>
    );
  }

  return (
    <div className="border border-line rounded-lg bg-surface-1 p-4">
      <div className="overflow-x-auto">
        <table className="min-w-full border-separate border-spacing-1">
          <thead>
            <tr>
              <th className="text-left text-[11px] text-ink-3 font-medium px-2 py-1">Cohort</th>
              <th className="text-right text-[11px] text-ink-3 font-medium px-2 py-1">Size</th>
              {Array.from({ length: maxPeriods }, (_, idx) => (
                <th
                  key={idx}
                  className="text-center text-[11px] text-ink-3 font-medium px-1 py-1"
                >
                  {columnLabel(granularity, idx)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, rowIndex) => (
              <tr key={row.cohort_start}>
                <td className="px-2 py-1 text-xs text-ink font-mono whitespace-nowrap">
                  {row.cohort_start.slice(0, 10)}
                </td>
                <td className="px-2 py-1 text-xs text-ink text-right font-mono tabular-nums whitespace-nowrap">
                  {row.cohort_size.toLocaleString()}
                </td>
                {row.periods.map((period) => {
                  const notElapsed = isNotElapsed(
                    row.cohort_start,
                    granularity,
                    period.offset,
                    endDate
                  );
                  const text = notElapsed ? '—' : `${Math.round(period.rate * 100)}%`;
                  const tooltipId = `retention-cell-tooltip-${rowIndex}-${period.offset}`;
                  const cohortDate = row.cohort_start.slice(0, 10);
                  return (
                    <td key={period.offset} className="p-0">
                      <div className="group relative">
                        <button
                          type="button"
                          aria-describedby={tooltipId}
                          aria-label={cellAriaLabel(
                            cohortDate,
                            granularity,
                            period.offset,
                            period.retained,
                            row.cohort_size,
                            period.rate,
                            notElapsed
                          )}
                          className={`h-8 min-w-[52px] rounded-sm border border-line/40 flex items-center justify-center text-[11px] font-mono tabular-nums ${
                            notElapsed
                              ? 'bg-zinc-800/40 text-ink-4'
                              : rateColor(period.rate)
                          } focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-spark/70`}
                        >
                          {text}
                        </button>
                        <RetentionTooltip
                          id={tooltipId}
                          cohortStart={cohortDate}
                          granularity={granularity}
                          offset={period.offset}
                          retained={period.retained}
                          cohortSize={row.cohort_size}
                          rate={period.rate}
                          notElapsed={notElapsed}
                        />
                      </div>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
