'use client';

import { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import dynamic from 'next/dynamic';
import { ComposableMap, Geographies, Geography, Sphere, Graticule } from 'react-simple-maps';
import topology from 'world-atlas/countries-110m.json';
import { ISO_NUMERIC_TO_A2 } from '@/lib/iso-numeric-to-alpha2';
import type { MetricRow } from '@/lib/api';
import { useFilters } from '@/hooks/useFilters';
import { cn, formatDuration } from '@/lib/utils';

// Lazy-load the flat choropleth map to keep the main bundle lean
const FlatMap = dynamic(
  () => import('./WorldMapFlat').then((m) => m.WorldMapFlat),
  {
    ssr: false,
    loading: () => (
      <div className="w-full aspect-[2/1] bg-surface-2 rounded-lg animate-skeleton" />
    ),
  },
);

interface WorldMapProps {
  data?: MetricRow[];
  loading?: boolean;
}

type MapMode = 'globe' | 'flat';

interface TooltipState {
  a2: string;
  x: number;
  y: number;
}

// Degrees of longitude rotated per second during auto-rotation
const AUTO_ROTATE_DEG_PER_S = 14;

// Intl country name formatter (created once, memoized outside component)
let _cnf: Intl.DisplayNames | null = null;
function getCountryName(a2: string): string {
  try {
    _cnf ??= new Intl.DisplayNames(['en'], { type: 'region' });
    return _cnf.of(a2) ?? a2;
  } catch {
    return a2;
  }
}

function fmtNum(n: number): string {
  return n.toLocaleString('en', { notation: 'compact', maximumFractionDigits: 1 });
}

export function WorldMap({ data = [], loading }: WorldMapProps) {
  const [mapMode, setMapMode] = useState<MapMode>('globe');
  // D3 geoOrthographic convention: [lambda (lon), phi (lat), gamma]
  const [rotation, setRotation] = useState<[number, number, number]>([0, -20, 0]);
  const [autoRotate, setAutoRotate] = useState(true);
  const [isDragging, setIsDragging] = useState(false);
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  const { filters } = useFilters();
  const selectedCountry = (filters['filter_country'] as string | undefined) ?? null;

  // Lookup maps derived from data
  const rowByA2 = useMemo(
    () => Object.fromEntries(data.map((row) => [row.value, row])),
    [data],
  );
  const maxVisitors = useMemo(
    () => Math.max(...data.map((d) => d.visitors), 1),
    [data],
  );

  // ── Auto-rotation ─────────────────────────────────────────────────────────
  useEffect(() => {
    if (!autoRotate || mapMode !== 'globe') return;
    let rafId: number;
    let lastTs: number | null = null;

    const tick = (ts: number) => {
      if (lastTs !== null) {
        const dt = ts - lastTs;
        setRotation((r) => [r[0] + (AUTO_ROTATE_DEG_PER_S * dt) / 1000, r[1], r[2]]);
      }
      lastTs = ts;
      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [autoRotate, mapMode]);

  // ── Drag-to-rotate ────────────────────────────────────────────────────────
  const isDraggingRef = useRef(false);
  const lastPosRef = useRef<[number, number] | null>(null);
  const resumeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const onPointerDown = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    isDraggingRef.current = true;
    setIsDragging(true);
    setTooltip(null);
    lastPosRef.current = [e.clientX, e.clientY];
    setAutoRotate(false);
    if (resumeTimerRef.current) clearTimeout(resumeTimerRef.current);
    e.currentTarget.setPointerCapture(e.pointerId);
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDraggingRef.current || !lastPosRef.current) return;
    const dx = e.clientX - lastPosRef.current[0];
    const dy = e.clientY - lastPosRef.current[1];
    setRotation(([rx, ry, rz]) => [
      rx + dx * 0.5,
      Math.max(-85, Math.min(85, ry - dy * 0.5)),
      rz,
    ]);
    lastPosRef.current = [e.clientX, e.clientY];
  }, []);

  const stopDrag = useCallback(() => {
    if (!isDraggingRef.current) return;
    isDraggingRef.current = false;
    setIsDragging(false);
    lastPosRef.current = null;
    resumeTimerRef.current = setTimeout(() => setAutoRotate(true), 2500);
  }, []);

  // ── Tooltip via event delegation ─────────────────────────────────────────
  // A single mousemove on the container reads data-a2 from the SVG path target.
  // No per-country event handlers needed.
  const onMouseMoveContainer = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (isDraggingRef.current) return;
    const target = e.target as SVGPathElement;
    const a2 = (target as unknown as HTMLElement).dataset?.a2;
    if (a2 && a2.length > 0) {
      const rect = e.currentTarget.getBoundingClientRect();
      setTooltip({ a2, x: e.clientX - rect.left, y: e.clientY - rect.top });
    } else {
      setTooltip(null);
    }
  }, []);

  const onMouseLeaveContainer = useCallback(() => setTooltip(null), []);

  // ── Choropleth fill ───────────────────────────────────────────────────────
  const getFill = useCallback(
    (a2: string | null): string => {
      if (!a2) return 'var(--surface-1)';
      const v = rowByA2[a2]?.visitors ?? 0;
      if (!v) return 'var(--surface-1)';
      if (a2 === selectedCountry) return 'var(--spark)';
      const alpha = (0.12 + 0.88 * (v / maxVisitors)).toFixed(2);
      return `rgba(0, 208, 132, ${alpha})`;
    },
    [rowByA2, maxVisitors, selectedCountry],
  );

  // ── Toggle button helper ──────────────────────────────────────────────────
  const toggleBtn = (mode: MapMode, label: string) => (
    <button
      key={mode}
      onClick={() => setMapMode(mode)}
      className={cn(
        'px-2.5 py-1 text-[11px] rounded-md transition-all duration-150',
        mapMode === mode
          ? 'bg-canvas text-ink font-medium border border-line'
          : 'text-ink-3 hover:text-ink-2',
      )}
    >
      {label}
    </button>
  );

  // ── Loading skeleton ──────────────────────────────────────────────────────
  if (loading) {
    return (
      <div className="bg-surface-1 border border-line rounded-lg p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="h-4 w-32 bg-surface-2 rounded animate-skeleton" />
          <div className="h-6 w-24 bg-surface-2 rounded-lg animate-skeleton" />
        </div>
        <div className="w-full max-w-[300px] aspect-square mx-auto rounded-full bg-surface-2 animate-skeleton" />
      </div>
    );
  }

  // ── Tooltip render ────────────────────────────────────────────────────────
  const renderTooltip = () => {
    if (!tooltip) return null;
    const row = rowByA2[tooltip.a2] ?? null;
    const name = getCountryName(tooltip.a2);

    // Clamp so tooltip doesn't overflow the right / bottom edge of container
    const TW = 172;
    const left = Math.min(tooltip.x + 14, 360 - TW - 4);
    const top = tooltip.y > 100 ? tooltip.y - 100 : tooltip.y + 16;

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

  // ── Render ────────────────────────────────────────────────────────────────
  return (
    <div className="bg-surface-1 border border-line rounded-lg p-6">
      {/* Header: title + Globe/Map toggle */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-[13px] font-medium text-ink">Visitors by Country</h3>
        <div className="flex bg-surface-2 p-0.5 rounded-lg border border-line">
          {toggleBtn('globe', 'Globe')}
          {toggleBtn('flat', 'Map')}
        </div>
      </div>

      {mapMode === 'globe' ? (
        <div
          className="relative w-full max-w-[360px] aspect-square mx-auto select-none touch-none"
          style={{ cursor: isDragging ? 'grabbing' : 'grab' }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={stopDrag}
          onPointerCancel={stopDrag}
          onMouseMove={onMouseMoveContainer}
          onMouseLeave={onMouseLeaveContainer}
        >
          <ComposableMap
            projection="geoOrthographic"
            projectionConfig={{ rotate: rotation, scale: 185 }}
            width={400}
            height={400}
            style={{ width: '100%', height: '100%' }}
          >
            {/* Ocean — darker than no-data countries for contrast */}
            <Sphere id="rsm-sphere" fill="var(--canvas)" stroke="var(--surface-2)" strokeWidth={0.8} />

            {/* Graticule grid lines */}
            <Graticule stroke="var(--surface-2)" strokeWidth={0.3} strokeOpacity={0.6} />

            {/* Countries — choropleth fill + data-a2 for tooltip delegation */}
            <Geographies geography={topology}>
              {({ geographies }) =>
                geographies.map((geo) => {
                  const a2 = ISO_NUMERIC_TO_A2[String(geo.id).padStart(3, '0')] ?? null;
                  const fill = getFill(a2);
                  const isSelected = a2 !== null && a2 === selectedCountry;
                  return (
                    <Geography
                      key={geo.rsmKey}
                      geography={geo}
                      fill={fill}
                      stroke={isSelected ? '#ffffff' : 'var(--canvas)'}
                      strokeWidth={isSelected ? 0.8 : 0.3}
                      // data-a2 read by the container's onMouseMove for tooltip
                      data-a2={a2 ?? ''}
                      style={{
                        default: { outline: 'none' },
                        hover: { outline: 'none' },
                        pressed: { outline: 'none' },
                      }}
                    />
                  );
                })
              }
            </Geographies>
          </ComposableMap>

          {/* Tooltip — positioned absolutely within the globe container */}
          {renderTooltip()}
        </div>
      ) : (
        <div className="w-full overflow-hidden">
          <FlatMap data={data} selectedCountry={selectedCountry} />
        </div>
      )}
      <p className="mt-3 text-[11px] text-ink-3">
        IP Geolocation by{' '}
        <a
          href="https://db-ip.com"
          target="_blank"
          rel="noopener noreferrer"
          className="text-ink-2 underline underline-offset-2 hover:text-ink"
        >
          DB-IP
        </a>
      </p>
    </div>
  );
}
