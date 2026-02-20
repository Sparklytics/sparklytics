'use client';

import { useState } from 'react';
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
import { useCreateWebsite } from '@/hooks/useWebsites';
import { TIMEZONE_GROUPS, getBrowserTimezone } from '@/lib/timezones';

interface CreateWebsiteDialogProps {
  open: boolean;
  onClose: () => void;
}

function navigate(path: string) {
  window.history.pushState({}, '', path);
  window.dispatchEvent(new PopStateEvent('popstate'));
}

export function CreateWebsiteDialog({ open, onClose }: CreateWebsiteDialogProps) {
  const [name, setName] = useState('');
  const [domain, setDomain] = useState('');
  const [timezone, setTimezone] = useState(getBrowserTimezone());
  const createWebsite = useCreateWebsite();

  function handleClose() {
    onClose();
    setName('');
    setDomain('');
    setTimezone(getBrowserTimezone());
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim() || !domain.trim()) return;
    const cleanDomain = domain.trim().replace(/^https?:\/\//, '').replace(/\/$/, '');
    const result = await createWebsite.mutateAsync({ name: name.trim(), domain: cleanDomain, timezone });
    handleClose();
    if (result?.data?.id) {
      navigate(`/dashboard/${result.data.id}`);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(isOpen) => { if (!isOpen) handleClose(); }}>
      <DialogContent className="bg-surface-1 border-line sm:rounded-xl max-w-md">
        <DialogHeader>
          <DialogTitle className="text-base font-semibold text-ink">Add website</DialogTitle>
          <DialogDescription className="text-xs text-ink-3">
            Add a new website to start tracking analytics.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          <label className="block">
            <span className="text-xs text-ink-2 mb-1 block">Name</span>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Website"
              maxLength={100}
              autoFocus
              className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink placeholder:text-ink-4 focus:outline-none focus:border-spark"
            />
          </label>

          <label className="block">
            <span className="text-xs text-ink-2 mb-1 block">Domain</span>
            <input
              value={domain}
              onChange={(e) => setDomain(e.target.value)}
              placeholder="example.com"
              className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink placeholder:text-ink-4 focus:outline-none focus:border-spark"
            />
          </label>

          <label className="block">
            <span className="text-xs text-ink-2 mb-1 block">Timezone</span>
            <select
              value={timezone}
              onChange={(e) => setTimezone(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:border-spark"
            >
              {Object.entries(TIMEZONE_GROUPS).map(([group, zones]) => (
                <optgroup key={group} label={group}>
                  {zones.map((tz) => (
                    <option key={tz} value={tz}>{tz}</option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>

          <DialogFooter>
            <Button type="button" variant="outline" size="sm" onClick={handleClose} className="text-xs">
              Cancel
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={createWebsite.isPending || !name.trim() || !domain.trim()}
              className="text-xs"
            >
              {createWebsite.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
              Add website
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
