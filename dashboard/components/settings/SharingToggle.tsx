'use client';

import { useState } from 'react';
import { Copy, Check, Link, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useEnableSharing, useDisableSharing } from '@/hooks/useShare';

interface SharingToggleProps {
  websiteId: string;
  /** Current share_id if sharing is already enabled, null otherwise */
  shareId: string | null;
}

export function SharingToggle({ websiteId, shareId: initialShareId }: SharingToggleProps) {
  const [shareId, setShareId] = useState<string | null>(initialShareId);
  const [copied, setCopied] = useState(false);

  const enable = useEnableSharing(websiteId);
  const disable = useDisableSharing(websiteId);

  const shareUrl = shareId
    ? `${typeof window !== 'undefined' ? window.location.origin : ''}/share/${shareId}`
    : null;

  async function handleEnable() {
    const result = await enable.mutateAsync();
    setShareId(result.data.share_id);
  }

  async function handleDisable() {
    await disable.mutateAsync();
    setShareId(null);
  }

  async function handleCopy() {
    if (!shareUrl) return;
    await navigator.clipboard.writeText(shareUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-ink">Public share link</p>
          <p className="text-xs text-ink-3 mt-1">
            Allow anyone with the link to view read-only analytics
          </p>
        </div>
        {shareId ? (
          <Button
            variant="outline"
            size="sm"
            onClick={handleDisable}
            disabled={disable.isPending}
            className="text-xs"
          >
            {disable.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            Disable
          </Button>
        ) : (
          <Button
            size="sm"
            onClick={handleEnable}
            disabled={enable.isPending}
            className="text-xs"
          >
            {enable.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            Enable sharing
          </Button>
        )}
      </div>

      {shareUrl && (
        <div className="flex items-center gap-2 bg-canvas border border-line rounded-md px-3 py-2">
          <Link className="w-4 h-4 text-ink-3 shrink-0" />
          <span className="text-xs text-ink-2 flex-1 truncate">{shareUrl}</span>
          <button onClick={handleCopy} className="shrink-0">
            {copied ? (
              <Check className="w-4 h-4 text-spark" />
            ) : (
              <Copy className="w-4 h-4 text-ink-3 hover:text-ink transition-colors" />
            )}
          </button>
        </div>
      )}
    </div>
  );
}
