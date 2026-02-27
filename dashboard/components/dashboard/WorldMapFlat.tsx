'use client';

import { useCallback, useMemo, useState } from 'react';
import { ComposableMap, Geographies, Geography } from 'react-simple-maps';
// world-atlas countries-110m.json is bundled at build time — no CDN request needed
import topology from 'world-atlas/countries-110m.json';
import { ISO_NUMERIC_TO_A2 } from '@/lib/iso-numeric-to-alpha2';
import { formatDuration } from '@/lib/utils';
import type { MetricRow } from '@/lib/api';

interface WorldMapFlatProps {
  data: MetricRow[];
  selectedCountry: string | null;
}

interface TooltipState {
  a2: string;
  x: number;
  y: number;
  containerWidth: number;
  containerHeight: number;
}

// Intl country name formatter (created once, memoized outside component)
let countryNameFormatter: Intl.DisplayNames | null = null;
function getCountryName(a2: string): string {
  try {
    countryNameFormatter ??= new Intl.DisplayNames(['en'], { type: 'region' });
    return countryNameFormatter.of(a2) ?? a2;
  } catch {
    return a2;
  }
}

function fmtNum(n: number): string {
  return n.toLocaleString('en', { notation: 'compact', maximumFractionDigits: 1 });
}

export function WorldMapFlat({ data, selectedCountry }: WorldMapFlatProps) {
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  // Build full row lookup: alpha-2 → MetricRow
  const rowByA2 = useMemo(
    () => Object.fromEntries(data.map((row) => [row.value, row])),
    [data],
  );

  const visitorsByA2 = useMemo(() => {
    const m: Record<string, number> = {};
    for (const row of data) {
      m[row.value] = row.visitors;
    }
    return m;
  }, [data]);

  const maxVisitors = useMemo(
    () => Math.max(...data.map((d) => d.visitors), 1),
    [data],
  );

  const getFill = (a2: string | null, visitors: number): string => {
    if (!a2 || !visitors) return 'var(--surface-2)';
    if (a2 === selectedCountry) return 'var(--spark)';
    // Linear scale: low-traffic → alpha 0.12, top-traffic → alpha 1.0
    const alpha = (0.12 + 0.88 * (visitors / maxVisitors)).toFixed(2);
    return `rgb(var(--spark-rgb) / ${alpha})`;
  };

  const getHoverFill = (a2: string | null, visitors: number): string => {
    if (!a2 || !visitors) return 'var(--surface-1)';
    if (a2 === selectedCountry) return 'var(--spark)';
    return 'rgb(var(--spark-rgb) / 0.65)';
  };

  // ── Tooltip via event delegation (pointer events support mouse + touch) ─
  const onPointerTrackContainer = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    const target = e.target as SVGPathElement;
    const a2 = (target as unknown as HTMLElement).dataset?.a2;
    if (a2 && a2.length > 0) {
      const rect = e.currentTarget.getBoundingClientRect();
      setTooltip({
        a2,
        x: e.clientX - rect.left,
        y: e.clientY - rect.top,
        containerWidth: rect.width,
        containerHeight: rect.height,
      });
    } else {
      setTooltip(null);
    }
  }, []);

  const onPointerLeaveContainer = useCallback(() => setTooltip(null), []);

  // ── Tooltip render ────────────────────────────────────────────────────────
  const renderTooltip = () => {
    if (!tooltip) return null;
    const row = rowByA2[tooltip.a2] ?? null;
    const name = getCountryName(tooltip.a2);

    const TW = 172;
    const TH = 100;
    const left = Math.min(tooltip.x + 14, Math.max(8, tooltip.containerWidth - TW - 8));
    const rawTop = tooltip.y > 80 ? tooltip.y - TH : tooltip.y + 16;
    const top = Math.max(8, Math.min(rawTop, tooltip.containerHeight - TH - 8));

    const stat = (label: string, value: string) => (
      <div className="flex justify-between gap-6">
        <span className="text-ink-3">{label}</span>
        <span className="font-mono tabular-nums text-ink">{value}</span>
      </div>
    );

    return (
      <div
        className="absolute pointer-events-none z-20 bg-surface-2 border border-line rounded-lg px-3 py-2 text-[12px] leading-relaxed"
        style={{ left, top, minWidth: TW }}
      >
        <p className="font-medium text-ink mb-1.5">{name}</p>
        <div className="space-y-0.5">
          {stat('Visitors', row ? fmtNum(row.visitors) : '—')}
          {row?.pageviews !== undefined && stat('Pageviews', fmtNum(row.pageviews!))}
          {row && stat('Bounce', `${(row.bounce_rate ?? 0).toFixed(1)}%`)}
          {row && stat('Avg. duration', row.avg_duration_seconds > 0 ? formatDuration(row.avg_duration_seconds) : '—')}
        </div>
      </div>
    );
  };

  return (
    <div
      className="relative"
      onPointerMove={onPointerTrackContainer}
      onPointerDown={onPointerTrackContainer}
      onPointerLeave={onPointerLeaveContainer}
      onPointerCancel={onPointerLeaveContainer}
    >
      <ComposableMap
        projectionConfig={{ scale: 130, center: [0, 10] }}
        style={{ width: '100%', height: 'auto', display: 'block' }}
      >
        <Geographies geography={topology}>
          {({ geographies }) =>
            geographies.map((geo) => {
              const a2 = ISO_NUMERIC_TO_A2[String(geo.id).padStart(3, '0')] ?? null;
              const visitors = a2 ? (visitorsByA2[a2] ?? 0) : 0;
              const fill = getFill(a2, visitors);
              const hoverFill = getHoverFill(a2, visitors);

              return (
                <Geography
                  key={geo.rsmKey}
                  geography={geo}
                  fill={fill}
                  stroke="var(--canvas)"
                  strokeWidth={0.4}
                  // data-a2 read by the container's pointer handlers for tooltip
                  data-a2={a2 ?? ''}
                  style={{
                    default: { outline: 'none', cursor: 'default' },
                    hover: { fill: hoverFill, outline: 'none', cursor: 'default' },
                    pressed: { outline: 'none' },
                  }}
                />
              );
            })
          }
        </Geographies>
      </ComposableMap>

      {/* Tooltip — positioned absolutely within the map container */}
      {renderTooltip()}
    </div>
  );
}
