'use client';

import { useState } from 'react';
import { Copy, Check } from 'lucide-react';

interface EmptyStateProps {
  websiteId: string;
  domain?: string;
}

export function EmptyState({ websiteId, domain }: EmptyStateProps) {
  const [copied, setCopied] = useState(false);

  const host = domain || window.location.host;
  const snippet = `<script
  async
  src="https://${host}/s.js"
  data-website-id="${websiteId}"
></script>`;

  function handleCopy() {
    navigator.clipboard.writeText(snippet);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="flex flex-col items-center justify-center py-16 px-6">
      <div className="max-w-lg w-full text-center">
        <h2 className="text-base font-semibold text-ink mb-2">No data yet</h2>
        <p className="text-sm text-ink-3 mb-8">
          Add the tracking snippet to your site. Once your first pageview arrives, data will appear here.
        </p>

        <div className="bg-surface-2 border border-line rounded-lg text-left relative group">
          <div className="flex items-center justify-between px-4 py-2 border-b border-line">
            <span className="text-xs text-ink-3">HTML</span>
            <button
              onClick={handleCopy}
              className="flex items-center gap-2 text-xs text-ink-3 hover:text-ink transition-colors"
            >
              {copied ? <Check className="w-4 h-4 text-spark" /> : <Copy className="w-4 h-4" />}
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
          <pre className="px-4 py-4 text-xs text-ink-2 font-mono overflow-x-auto whitespace-pre">
            {snippet}
          </pre>
        </div>

        <p className="text-xs text-ink-4 mt-4">
          Paste this snippet in your{' '}
          <code className="font-mono text-ink-3">&lt;head&gt;</code> tag.
        </p>

        <div className="mt-6 flex items-center justify-center gap-4">
          <button
            onClick={() => {
              window.history.pushState({}, '', '/onboarding');
              window.dispatchEvent(new PopStateEvent('popstate'));
            }}
            className="text-xs text-spark hover:underline"
          >
            Open setup wizard
          </button>
          <button
            onClick={() => {
              window.history.pushState({}, '', `/settings/${websiteId}`);
              window.dispatchEvent(new PopStateEvent('popstate'));
            }}
            className="text-xs text-ink-3 hover:text-ink transition-colors"
          >
            Go to settings
          </button>
        </div>
      </div>
    </div>
  );
}
