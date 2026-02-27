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
import type { CampaignLink } from '@/lib/api';

const inputClass =
  'w-full px-3 py-2 text-sm bg-canvas border border-line rounded-md text-ink placeholder:text-ink-4 focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark';

const labelClass = 'block text-xs font-medium text-ink-3 mb-1';

interface EditLinkDialogProps {
  link: CampaignLink | null;
  isPending: boolean;
  onSave: (linkId: string, payload: {
    name: string;
    destination_url: string;
    utm_source: string | null;
    utm_medium: string | null;
    utm_campaign: string | null;
  }) => void;
  onClose: () => void;
}

export function EditLinkDialog({ link, isPending, onSave, onClose }: EditLinkDialogProps) {
  const [name, setName] = useState('');
  const [destinationUrl, setDestinationUrl] = useState('');
  const [utmSource, setUtmSource] = useState('');
  const [utmMedium, setUtmMedium] = useState('');
  const [utmCampaign, setUtmCampaign] = useState('');

  useEffect(() => {
    if (link) {
      setName(link.name);
      setDestinationUrl(link.destination_url);
      setUtmSource(link.utm_source ?? '');
      setUtmMedium(link.utm_medium ?? '');
      setUtmCampaign(link.utm_campaign ?? '');
    }
  }, [link]);

  function handleSubmit() {
    if (!link || !name.trim() || !destinationUrl.trim()) return;
    onSave(link.id, {
      name: name.trim(),
      destination_url: destinationUrl.trim(),
      utm_source: utmSource.trim() || null,
      utm_medium: utmMedium.trim() || null,
      utm_campaign: utmCampaign.trim() || null,
    });
  }

  return (
    <Dialog open={!!link} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-lg max-w-md">
        <DialogHeader>
          <DialogTitle className="text-ink">Edit Campaign Link</DialogTitle>
          <DialogDescription className="sr-only">Edit the campaign link details</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <label className="block">
            <span className={labelClass}>Name</span>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Link name"
              className={inputClass}
            />
          </label>

          <label className="block">
            <span className={labelClass}>Destination URL</span>
            <input
              value={destinationUrl}
              onChange={(e) => setDestinationUrl(e.target.value)}
              placeholder="https://example.com/landing"
              className={inputClass}
            />
          </label>

          <label className="block">
            <span className={labelClass}>UTM Source (optional)</span>
            <input
              value={utmSource}
              onChange={(e) => setUtmSource(e.target.value)}
              placeholder="e.g. newsletter"
              className={inputClass}
            />
          </label>

          <label className="block">
            <span className={labelClass}>UTM Medium (optional)</span>
            <input
              value={utmMedium}
              onChange={(e) => setUtmMedium(e.target.value)}
              placeholder="e.g. email"
              className={inputClass}
            />
          </label>

          <label className="block">
            <span className={labelClass}>UTM Campaign (optional)</span>
            <input
              value={utmCampaign}
              onChange={(e) => setUtmCampaign(e.target.value)}
              placeholder="e.g. spring-sale"
              className={inputClass}
            />
          </label>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button onClick={handleSubmit} disabled={isPending || !name.trim() || !destinationUrl.trim()}>
            {isPending && <Loader2 className="w-4 h-4 animate-spin" />}
            Save Changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
