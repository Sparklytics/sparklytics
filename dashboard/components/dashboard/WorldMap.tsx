'use client';

import { useEffect, useRef } from 'react';
import createGlobe from 'cobe';
import type { MetricRow } from '@/lib/api';

const COUNTRY_LOCATIONS: Record<string, [number, number]> = {
    US: [37.0902, -95.7129],
    GB: [55.3781, -3.4360],
    DE: [51.1657, 10.4515],
    FR: [46.2276, 2.2137],
    IN: [20.5937, 78.9629],
    CA: [56.1304, -106.3468],
    AU: [-25.2744, 133.7751],
    BR: [-14.2350, -51.9253],
    CN: [35.8617, 104.1954],
    JP: [36.2048, 138.2529],
    KR: [35.9078, 127.7669],
    MX: [23.6345, -102.5528],
    IT: [41.8719, 12.5674],
    ES: [40.4637, -3.7492],
    PL: [51.9194, 19.1451],
    RU: [61.5240, 105.3188],
    NL: [52.1326, 5.2913],
    TR: [38.9637, 35.2433],
    ZA: [-30.5595, 22.9375],
    ID: [-0.7893, 113.9213],
    SG: [1.3521, 103.8198],
    SE: [60.1282, 18.6435],
    NO: [60.4720, 8.4689],
    FI: [61.9241, 25.7482],
    DK: [56.2639, 9.5018]
};

interface WorldMapProps {
    data?: MetricRow[];
    loading?: boolean;
}

export function WorldMap({ data = [], loading }: WorldMapProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        let phi = 0;
        if (!canvasRef.current) return;

        const maxVisitors = Math.max(...data.map(d => d.visitors), 1);
        const markers = data
            .map((row) => {
                const coords = COUNTRY_LOCATIONS[row.value];
                if (!coords) return null;
                return {
                    location: coords,
                    size: 0.1 * (row.visitors / maxVisitors) + 0.03,
                };
            })
            .filter(Boolean) as { location: [number, number]; size: number }[];

        const isDark = typeof window !== 'undefined' && document.documentElement.classList.contains('dark');

        const globe = createGlobe(canvasRef.current, {
            devicePixelRatio: 2,
            width: 800,
            height: 800,
            phi: 0,
            theta: 0,
            dark: isDark ? 1 : 0,
            diffuse: 1.2,
            mapSamples: 16000,
            mapBrightness: 3,
            baseColor: isDark ? [0.15, 0.15, 0.15] : [0.95, 0.95, 0.95],
            markerColor: [0.1, 0.8, 0.4],
            glowColor: isDark ? [0.05, 0.05, 0.05] : [0.95, 0.95, 0.95],
            markers,
            onRender: (state) => {
                state.phi = phi;
                phi += 0.003;
            },
        });

        return () => {
            globe.destroy();
        };
    }, [data]);

    if (loading) {
        return (
            <div className="bg-surface-1 border border-line rounded-lg p-6 flex flex-col items-center justify-center">
                <div className="w-full max-w-[300px] aspect-square rounded-full bg-surface-2 animate-pulse" />
            </div>
        );
    }

    return (
        <div className="bg-surface-1 border border-line rounded-lg p-6 flex items-center justify-center overflow-hidden h-full">
            <div className="relative w-full max-w-[400px] aspect-square">
                <canvas
                    ref={canvasRef}
                    style={{ width: '100%', height: '100%', objectFit: 'contain' }}
                />
            </div>
        </div>
    );
}
