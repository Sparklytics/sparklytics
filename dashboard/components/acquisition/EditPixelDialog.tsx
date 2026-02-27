'use client';

import { useState, useEffect } from 'react';
import { Loader2 } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import type { TrackingPixel } from '@/lib/api';

const inputClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-4 focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

const labelClass = 'block text-xs font-medium text-ink-3 mb-1';

interface EditPixelDialogProps {
  pixel: TrackingPixel | null;
  isPending: boolean;
  onSave: (pixelId: string, payload: {
    name: string;
    default_url: string | null;
  }) => void;
  onClose: () => void;
}

export function EditPixelDialog({ pixel, isPending, onSave, onClose }: EditPixelDialogProps) {
  const [name, setName] = useState('');
  const [defaultUrl, setDefaultUrl] = useState('');

  useEffect(() => {
    if (pixel) {
      setName(pixel.name);
      setDefaultUrl(pixel.default_url ?? '');
    }
  }, [pixel]);

  function handleSubmit() {
    if (!pixel || !name.trim()) return;
    onSave(pixel.id, {
      name: name.trim(),
      default_url: defaultUrl.trim() || null,
    });
  }

  return (
    <Dialog open={!!pixel} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-lg max-w-md">
        <DialogHeader>
          <DialogTitle className="text-ink">Edit Tracking Pixel</DialogTitle>
          <DialogDescription className="sr-only">Edit the tracking pixel details</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <label className="block">
            <span className={labelClass}>Name</span>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Pixel name"
              className={inputClass}
            />
          </label>

          <label className="block">
            <span className={labelClass}>Default URL (optional)</span>
            <input
              value={defaultUrl}
              onChange={(e) => setDefaultUrl(e.target.value)}
              placeholder="https://example.com/page"
              className={inputClass}
            />
          </label>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button onClick={handleSubmit} disabled={isPending || !name.trim()}>
            {isPending && <Loader2 className="w-4 h-4 animate-spin" />}
            Save Changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
