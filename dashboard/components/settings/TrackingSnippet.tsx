'use client';

import { useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface TrackingSnippetProps {
  websiteId: string;
}

export function TrackingSnippet({ websiteId }: TrackingSnippetProps) {
  const [copied, setCopied] = useState(false);

  const snippet = `<script defer src="/s.js" data-website-id="${websiteId}"></script>`;

  async function handleCopy() {
    await navigator.clipboard.writeText(snippet);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-sm text-ink-2">Tracking snippet</span>
        <Button variant="ghost" size="sm" onClick={handleCopy} className="gap-1 text-xs">
          {copied ? <Check className="w-3 h-3 text-spark" /> : <Copy className="w-3 h-3" />}
          {copied ? 'Copied' : 'Copy'}
        </Button>
      </div>
      <pre className="bg-canvas border border-line rounded-md p-3 text-xs text-ink-2 overflow-x-auto whitespace-pre-wrap break-all">
        {snippet}
      </pre>
      <p className="text-xs text-ink-3">
        Paste this snippet inside the{' '}
        <code className="text-ink-2">&lt;head&gt;</code> tag of your website.
      </p>
    </div>
  );
}
