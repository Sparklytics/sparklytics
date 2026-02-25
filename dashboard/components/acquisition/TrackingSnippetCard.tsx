'use client';

import { Copy } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface TrackingSnippetCardProps {
  snippet: string;
}

export function TrackingSnippetCard({ snippet }: TrackingSnippetCardProps) {
  return (
    <div className="border border-line rounded-lg bg-surface-1 p-3">
      <p className="text-xs font-medium text-ink-2 mb-2">Tracking snippet</p>
      <pre className="text-xs text-ink-2 bg-surface-2 border border-line rounded p-2 overflow-x-auto">
        {snippet}
      </pre>
      <div className="flex justify-end mt-2">
        <Button type="button" size="sm" variant="outline" className="h-7 px-2 text-xs" onClick={() => navigator.clipboard.writeText(snippet)}>
          <Copy className="w-3 h-3 mr-1" />
          Copy Snippet
        </Button>
      </div>
    </div>
  );
}
