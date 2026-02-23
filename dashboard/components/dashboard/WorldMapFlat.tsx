'use client';

import { useMemo } from 'react';
import { ComposableMap, Geographies, Geography } from 'react-simple-maps';
// world-atlas countries-110m.json is bundled at build time — no CDN request needed
import topology from 'world-atlas/countries-110m.json';
import { ISO_NUMERIC_TO_A2 } from '@/lib/iso-numeric-to-alpha2';
import type { MetricRow } from '@/lib/api';

interface WorldMapFlatProps {
  data: MetricRow[];
  selectedCountry: string | null;
}

export function WorldMapFlat({ data, selectedCountry }: WorldMapFlatProps) {
  // Build visitor lookup: alpha-2 → count
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
    return `rgba(0, 208, 132, ${alpha})`;
  };

  const getHoverFill = (a2: string | null, visitors: number): string => {
    if (!a2 || !visitors) return 'var(--surface-1)';
    if (a2 === selectedCountry) return 'var(--spark)';
    return 'rgba(0, 208, 132, 0.65)';
  };

  return (
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
                style={{
                  default: { outline: 'none' },
                  hover: { fill: hoverFill, outline: 'none', cursor: 'default' },
                  pressed: { outline: 'none' },
                }}
              />
            );
          })
        }
      </Geographies>
    </ComposableMap>
  );
}
