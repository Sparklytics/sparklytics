'use client';

import { useState } from 'react';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { CreateTrackingPixelPayload } from '@/lib/api';

interface CreatePixelDialogProps {
  isPending: boolean;
  onCreate: (payload: CreateTrackingPixelPayload) => void;
}

export function CreatePixelDialog({ isPending, onCreate }: CreatePixelDialogProps) {
  const [name, setName] = useState('');
  const [defaultUrl, setDefaultUrl] = useState('');

  function submit() {
    if (!name.trim()) return;
    onCreate({
      name: name.trim(),
      default_url: defaultUrl.trim() || undefined,
    });
    setName('');
    setDefaultUrl('');
  }

  return (
    <div className="border border-line rounded-lg bg-surface-1 p-3 space-y-2">
      <p className="text-xs font-medium text-ink-2">Create pixel</p>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        <Input placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} />
        <Input
          placeholder="Default URL (optional)"
          value={defaultUrl}
          onChange={(e) => setDefaultUrl(e.target.value)}
        />
      </div>
      <div className="flex justify-end">
        <Button type="button" size="sm" className="text-xs gap-1" disabled={isPending} onClick={submit}>
          <Plus className="w-3.5 h-3.5" />
          Create Pixel
        </Button>
      </div>
    </div>
  );
}
