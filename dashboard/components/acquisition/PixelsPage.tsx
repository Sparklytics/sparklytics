'use client';

import { useEffect, useMemo, useState } from 'react';
import { Copy, Image as ImageIcon, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { TrackingPixel } from '@/lib/api';
import {
  useCreateTrackingPixel,
  useDeleteTrackingPixel,
  useTrackingPixels,
  useUpdateTrackingPixel,
} from '@/hooks/useTrackingPixels';
import { CreatePixelDialog } from './CreatePixelDialog';
import { EditPixelDialog } from './EditPixelDialog';
import { TrackingSnippetCard } from './TrackingSnippetCard';

interface PixelsPageProps {
  websiteId: string;
}

function SummaryCards({
  totalPixels,
  totalViews,
  totalVisitors,
  isLoading,
}: {
  totalPixels: number;
  totalViews: number;
  totalVisitors: number;
  isLoading: boolean;
}) {
  const cards = [
    { label: 'Total Pixels', value: totalPixels },
    { label: 'Total Views', value: totalViews },
    { label: 'Total Visitors', value: totalVisitors },
  ];

  return (
    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
      {cards.map(({ label, value }) => (
        <div key={label} className="border border-line rounded-lg bg-surface-1 p-4">
          <p className="text-[11px] text-ink-3 uppercase tracking-[0.07em] font-medium mb-1">
            {label}
          </p>
          {isLoading ? (
            <div className="animate-pulse bg-surface-2 rounded h-6 w-16" />
          ) : (
            <p className="text-2xl font-mono tabular-nums text-ink font-semibold">
              {value.toLocaleString()}
            </p>
          )}
        </div>
      ))}
    </div>
  );
}

function SkeletonRows() {
  return (
    <>
      {[0, 1, 2].map((i) => (
        <tr key={i} className="border-t border-line">
          <td className="px-3 py-3">
            <div className="animate-pulse space-y-1.5">
              <div className="bg-surface-2 rounded h-3.5 w-32" />
              <div className="bg-surface-2 rounded h-3 w-48" />
            </div>
          </td>
          <td className="px-3 py-3">
            <div className="animate-pulse bg-surface-2 rounded h-3 w-28" />
          </td>
          <td className="px-3 py-3">
            <div className="animate-pulse bg-surface-2 rounded h-5 w-24" />
          </td>
          <td className="px-3 py-3">
            <div className="animate-pulse bg-surface-2 rounded h-5 w-14" />
          </td>
          <td className="px-3 py-3">
            <div className="animate-pulse bg-surface-2 rounded h-7 w-48" />
          </td>
        </tr>
      ))}
    </>
  );
}

function EmptyState() {
  return (
    <tr>
      <td colSpan={5}>
        <div className="flex flex-col items-center justify-center gap-3 py-12 px-4 text-center">
          <div className="w-10 h-10 rounded-lg bg-surface-2 border border-line flex items-center justify-center">
            <ImageIcon className="w-5 h-5 text-ink-3" />
          </div>
          <div className="space-y-1">
            <p className="text-sm font-medium text-ink">No tracking pixels yet</p>
            <p className="text-xs text-ink-3 max-w-xs">
              Tracking pixels are 1x1 image beacons that let you measure opens in emails and
              documentation pages where JavaScript is unavailable.
            </p>
          </div>
          <p className="text-xs text-ink-3 border border-line rounded px-2.5 py-1 bg-surface-2">
            Click <span className="text-ink font-medium">Create Pixel</span> above to get started.
          </p>
        </div>
      </td>
    </tr>
  );
}

function PixelRow({
  pixel,
  isSelected,
  onDelete,
  onToggle,
  onEdit,
  onPreview,
}: {
  pixel: TrackingPixel;
  isSelected: boolean;
  onDelete: (id: string) => void;
  onToggle: (id: string, active: boolean) => void;
  onEdit: (pixel: TrackingPixel) => void;
  onPreview: (pixel: TrackingPixel) => void;
}) {
  return (
    <tr
      className={`border-t border-line align-top ${
        isSelected ? 'border-l-2 border-l-spark bg-spark/[0.02]' : ''
      }`}
    >
      <td className="px-3 py-2">
        <p className="text-sm text-ink font-medium">{pixel.name}</p>
        <p className="text-xs text-ink-3 mt-0.5">{pixel.default_url ?? 'No default URL'}</p>
      </td>
      <td className="px-3 py-2">
        <div className="text-xs text-ink-3">
          <span className="font-mono tabular-nums font-medium text-ink">{pixel.views ?? 0}</span> views
          {' · '}
          <span className="font-mono tabular-nums font-medium text-ink">{pixel.unique_visitors ?? 0}</span> visitors
        </div>
      </td>
      <td className="px-3 py-2">
        <code className="text-xs text-ink bg-surface-2 px-1.5 py-0.5 rounded">{pixel.pixel_key}</code>
      </td>
      <td className="px-3 py-2">
        <span
          className={`text-xs px-2 py-1 rounded-sm border ${
            pixel.is_active ? 'border-spark text-spark' : 'border-line text-ink-3'
          }`}
        >
          {pixel.is_active ? 'Active' : 'Inactive'}
        </span>
      </td>
      <td className="px-3 py-2">
        <div className="flex items-center gap-1 flex-wrap">
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => navigator.clipboard.writeText(pixel.snippet)}
            title="Copy Snippet"
          >
            <Copy className="w-3 h-3 mr-1" />
            Copy
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => onPreview(pixel)}
            title="View Snippet"
          >
            View
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => onEdit(pixel)}
          >
            Edit
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => onToggle(pixel.id, !pixel.is_active)}
          >
            {pixel.is_active ? 'Disable' : 'Enable'}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs text-down"
            onClick={() => onDelete(pixel.id)}
            title="Delete"
          >
            <Trash2 className="w-3 h-3" />
          </Button>
        </div>
      </td>
    </tr>
  );
}

export function PixelsPage({ websiteId }: PixelsPageProps) {
  const { data, isLoading } = useTrackingPixels(websiteId);
  const createPixel = useCreateTrackingPixel(websiteId);
  const updatePixel = useUpdateTrackingPixel(websiteId);
  const deletePixel = useDeleteTrackingPixel(websiteId);
  const pixels = useMemo(() => data?.data ?? [], [data]);
  const [selectedPixelId, setSelectedPixelId] = useState<string | null>(null);
  const [editingPixel, setEditingPixel] = useState<TrackingPixel | null>(null);

  const totalViews = useMemo(
    () => pixels.reduce((sum, p) => sum + (p.views ?? 0), 0),
    [pixels],
  );
  const totalVisitors = useMemo(
    () => pixels.reduce((sum, p) => sum + (p.unique_visitors ?? 0), 0),
    [pixels],
  );

  useEffect(() => {
    if (pixels.length === 0) {
      setSelectedPixelId(null);
      return;
    }
    if (!selectedPixelId || !pixels.some((pixel) => pixel.id === selectedPixelId)) {
      setSelectedPixelId(pixels[0].id);
    }
  }, [pixels, selectedPixelId]);

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">Tracking Pixels</h2>
        <p className="text-xs text-ink-3 mt-0.5">
          Use 1x1 image beacons for email and documentation channels without JavaScript.
        </p>
      </div>

      <SummaryCards
        totalPixels={pixels.length}
        totalViews={totalViews}
        totalVisitors={totalVisitors}
        isLoading={isLoading}
      />

      <CreatePixelDialog
        isPending={createPixel.isPending}
        onCreate={(payload) => createPixel.mutate(payload)}
      />

      <div className="border border-line rounded-lg bg-surface-1 overflow-x-auto">
        <table className="w-full text-left min-w-[640px]">
          <thead className="bg-surface-2">
            <tr className="text-xs text-ink-3">
              <th className="px-3 py-2 font-medium">Pixel</th>
              <th className="px-3 py-2 font-medium">Stats</th>
              <th className="px-3 py-2 font-medium">Key</th>
              <th className="px-3 py-2 font-medium">Status</th>
              <th className="px-3 py-2 font-medium">Actions</th>
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              <SkeletonRows />
            ) : pixels.length === 0 ? (
              <EmptyState />
            ) : (
              pixels.map((pixel) => (
                <PixelRow
                  key={pixel.id}
                  pixel={pixel}
                  isSelected={pixel.id === selectedPixelId}
                  onDelete={(id) => deletePixel.mutate(id)}
                  onEdit={setEditingPixel}
                  onPreview={(selected) => setSelectedPixelId(selected.id)}
                  onToggle={(id, active) =>
                    updatePixel.mutate({ pixelId: id, payload: { is_active: active } })
                  }
                />
              ))
            )}
          </tbody>
        </table>
      </div>

      {selectedPixelId ? (
        <TrackingSnippetCard
          snippet={pixels.find((pixel) => pixel.id === selectedPixelId)?.snippet ?? ''}
        />
      ) : null}

      <EditPixelDialog
        pixel={editingPixel}
        isPending={updatePixel.isPending}
        onSave={(pixelId, payload) => {
          updatePixel.mutate({ pixelId, payload }, { onSuccess: () => setEditingPixel(null) });
        }}
        onClose={() => setEditingPixel(null)}
      />
    </div>
  );
}
