'use client';

import { useEffect, useMemo, useState } from 'react';
import { Copy, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { TrackingPixel } from '@/lib/api';
import {
  useCreateTrackingPixel,
  useDeleteTrackingPixel,
  useTrackingPixels,
  useUpdateTrackingPixel,
} from '@/hooks/useTrackingPixels';
import { CreatePixelDialog } from './CreatePixelDialog';
import { TrackingSnippetCard } from './TrackingSnippetCard';

interface PixelsPageProps {
  websiteId: string;
}

function PixelRow({
  pixel,
  onDelete,
  onToggle,
  onEdit,
  onPreview,
}: {
  pixel: TrackingPixel;
  onDelete: (id: string) => void;
  onToggle: (id: string, active: boolean) => void;
  onEdit: (pixel: TrackingPixel) => void;
  onPreview: (pixel: TrackingPixel) => void;
}) {
  return (
    <tr className="border-t border-line align-top">
      <td className="px-3 py-2">
        <p className="text-sm text-ink font-medium">{pixel.name}</p>
        <p className="text-xs text-ink-3 mt-0.5">{pixel.default_url ?? 'No default URL'}</p>
      </td>
      <td className="px-3 py-2">
        <div className="text-xs text-ink-3">
          <span className="font-medium text-ink">{pixel.views ?? 0}</span> views
          {' · '}
          <span className="font-medium text-ink">{pixel.unique_visitors ?? 0}</span> visitors
        </div>
      </td>
      <td className="px-3 py-2">
        <code className="text-xs text-ink bg-surface-2 px-1.5 py-0.5 rounded">{pixel.pixel_key}</code>
      </td>
      <td className="px-3 py-2">
        <span className={`text-xs px-2 py-1 rounded border ${pixel.is_active ? 'border-spark text-spark' : 'border-line text-ink-3'}`}>
          {pixel.is_active ? 'Active' : 'Inactive'}
        </span>
      </td>
      <td className="px-3 py-2">
        <div className="flex items-center gap-1">
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => navigator.clipboard.writeText(pixel.snippet)}
          >
            <Copy className="w-3 h-3 mr-1" />
            Copy Snippet
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => onPreview(pixel)}
          >
            View Snippet
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
            className="h-7 px-2 text-xs text-red-400"
            onClick={() => onDelete(pixel.id)}
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

  useEffect(() => {
    if (pixels.length === 0) {
      setSelectedPixelId(null);
      return;
    }
    if (!selectedPixelId || !pixels.some((pixel) => pixel.id === selectedPixelId)) {
      setSelectedPixelId(pixels[0].id);
    }
  }, [pixels, selectedPixelId]);

  function onEdit(pixel: TrackingPixel) {
    const name = window.prompt('Pixel name', pixel.name);
    if (name === null || !name.trim()) return;

    const defaultUrl = window.prompt('Default URL (optional)', pixel.default_url ?? '');
    if (defaultUrl === null) return;

    updatePixel.mutate({
      pixelId: pixel.id,
      payload: {
        name: name.trim(),
        default_url: defaultUrl.trim() || null,
      },
    });
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">Tracking Pixels</h2>
        <p className="text-xs text-ink-3 mt-0.5">
          Use 1x1 image beacons for email and documentation channels without JavaScript.
        </p>
      </div>

      <CreatePixelDialog
        isPending={createPixel.isPending}
        onCreate={(payload) => createPixel.mutate(payload)}
      />

      <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
        <table className="w-full text-left">
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
              <tr>
                <td className="px-3 py-6 text-sm text-ink-3" colSpan={5}>
                  Loading tracking pixels...
                </td>
              </tr>
            ) : pixels.length === 0 ? (
              <tr>
                <td className="px-3 py-6 text-sm text-ink-3" colSpan={5}>
                  No tracking pixels created yet.
                </td>
              </tr>
            ) : (
              pixels.map((pixel) => (
                <PixelRow
                  key={pixel.id}
                  pixel={pixel}
                  onDelete={(id) => deletePixel.mutate(id)}
                  onEdit={onEdit}
                  onPreview={(selected) => setSelectedPixelId(selected.id)}
                  onToggle={(id, active) => updatePixel.mutate({ pixelId: id, payload: { is_active: active } })}
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
    </div>
  );
}
