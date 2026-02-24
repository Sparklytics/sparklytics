'use client';

import { RetentionCohortRow, RetentionGranularity } from '@/lib/api';
import { periodLabel } from '@/lib/retention-utils';
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
    // Fix: normalise to day 1 before adding months to avoid JS date overflow
    // e.g. Jan 31 + 1 month would otherwise become March 3
    date.setUTCDate(1);
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
  // Use >= so that a period starting exactly on the endDate boundary is also
  // treated as not-yet-elapsed (no data could be in it yet).
  return periodStart.getTime() >= end.getTime();
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
  const period = periodLabel(granularity, offset);
  if (notElapsed) {
    return `Cohort ${cohortStart}, ${period}, not yet elapsed`;
  }
  return `Cohort ${cohortStart}, ${period}, ${retained} of ${cohortSize} retained, ${Math.round(
    rate * 100
  )} percent`;
}

// Color tiers using the --spark design token instead of hardcoded emerald shades.
// text-canvas is used on the brightest cells so dark text stays readable.
function rateColor(rate: number): string {
  if (rate <= 0)   return 'bg-surface-2 text-ink-4';
  if (rate <= 0.2) return 'bg-spark/10 text-ink-2';
  if (rate <= 0.4) return 'bg-spark/25 text-ink';
  if (rate <= 0.6) return 'bg-spark/45 text-ink';
  if (rate <= 0.8) return 'bg-spark/65 text-ink';
  return                  'bg-spark/80 text-canvas';
}

const LEGEND_BANDS = [
  { label: '0%',    color: 'bg-surface-2' },
  { label: '20%',   color: 'bg-spark/10' },
  { label: '40%',   color: 'bg-spark/25' },
  { label: '60%',   color: 'bg-spark/45' },
  { label: '80%',   color: 'bg-spark/65' },
  { label: '100%',  color: 'bg-spark/80' },
];

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
    <div className="border border-line rounded-lg bg-surface-1 p-4 space-y-3">
      <div className="overflow-x-auto">
        <table className="min-w-full border-separate border-spacing-1">
          <caption className="sr-only">Cohort retention heatmap</caption>
          <thead>
            <tr>
              <th
                scope="col"
                className="sticky left-0 z-10 bg-surface-1 text-left text-[11px] text-ink-3 font-medium px-2 py-1"
              >
                Cohort
              </th>
              <th
                scope="col"
                className="sticky left-[5.5rem] z-10 bg-surface-1 text-right text-[11px] text-ink-3 font-medium px-2 py-1"
              >
                Size
              </th>
              {Array.from({ length: maxPeriods }, (_, idx) => (
                <th
                  key={idx}
                  scope="col"
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
                <th
                  scope="row"
                  className="sticky left-0 z-10 bg-surface-1 px-2 py-1 text-xs text-ink font-mono whitespace-nowrap"
                >
                  {row.cohort_start.slice(0, 10)}
                </th>
                <td className="sticky left-[5.5rem] z-10 bg-surface-1 px-2 py-1 text-xs text-ink text-right font-mono tabular-nums whitespace-nowrap">
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
                              ? 'bg-surface-2/50 text-ink-4'
                              : rateColor(period.rate)
                          } focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-spark`}
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

      {/* Color legend */}
      <div className="flex items-center gap-3 pt-1">
        <span className="text-[10px] text-ink-4">Retention</span>
        <div className="flex items-center gap-1">
          {LEGEND_BANDS.map((band) => (
            <div key={band.label} className="flex flex-col items-center gap-0.5">
              <div className={`w-6 h-3 rounded-sm border border-line/40 ${band.color}`} />
              <span className="text-[9px] text-ink-4 tabular-nums">{band.label}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
