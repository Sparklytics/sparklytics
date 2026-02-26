'use client';

import { useState } from 'react';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { CreateCampaignLinkPayload } from '@/lib/api';

interface CreateLinkDialogProps {
  isPending: boolean;
  onCreate: (payload: CreateCampaignLinkPayload) => void;
}

export function CreateLinkDialog({ isPending, onCreate }: CreateLinkDialogProps) {
  const [name, setName] = useState('');
  const [destinationUrl, setDestinationUrl] = useState('');
  const [utmSource, setUtmSource] = useState('');
  const [utmMedium, setUtmMedium] = useState('');
  const [utmCampaign, setUtmCampaign] = useState('');

  function submit() {
    if (!name.trim() || !destinationUrl.trim()) return;
    onCreate({
      name: name.trim(),
      destination_url: destinationUrl.trim(),
      utm_source: utmSource.trim() || undefined,
      utm_medium: utmMedium.trim() || undefined,
      utm_campaign: utmCampaign.trim() || undefined,
    });
    setName('');
    setDestinationUrl('');
    setUtmSource('');
    setUtmMedium('');
    setUtmCampaign('');
  }

  return (
    <div className="border border-line rounded-lg bg-surface-1 p-3 space-y-2">
      <p className="text-xs font-medium text-ink-2">Create link</p>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-2">
        <Input placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} />
        <Input
          placeholder="Destination URL"
          value={destinationUrl}
          onChange={(e) => setDestinationUrl(e.target.value)}
        />
        <Input placeholder="UTM source" value={utmSource} onChange={(e) => setUtmSource(e.target.value)} />
        <Input placeholder="UTM medium" value={utmMedium} onChange={(e) => setUtmMedium(e.target.value)} />
        <Input placeholder="UTM campaign" value={utmCampaign} onChange={(e) => setUtmCampaign(e.target.value)} />
      </div>
      <div className="flex justify-end">
        <Button type="button" size="sm" className="text-xs gap-1" disabled={isPending} onClick={submit}>
          <Plus className="w-3.5 h-3.5" />
          Create Link
        </Button>
      </div>
    </div>
  );
}
