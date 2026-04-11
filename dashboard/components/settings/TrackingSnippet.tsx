'use client';

import { useEffect, useMemo, useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  buildFirstPartyTrackingSnippet,
  buildTrackingSnippet,
  DEFAULT_FIRST_PARTY_PROXY_PATH,
  extractProxyPath,
  extractTrackingBase,
  inferTrackingMode,
  normalizeProxyPath,
  type TrackingMode,
} from '@/lib/tracking';

interface TrackingSnippetProps {
  websiteId: string;
  snippet?: string;
}

const MODE_OPTIONS: { id: TrackingMode; label: string; hint: string }[] = [
  {
    id: 'direct',
    label: 'Direct analytics subdomain',
    hint: 'Best for generic installs where analytics runs on its own public origin.',
  },
  {
    id: 'first_party',
    label: 'First-party proxy path',
    hint: 'Recommended behind Cloudflare or aggressive blockers on public content sites.',
  },
];

export function TrackingSnippet({ websiteId, snippet }: TrackingSnippetProps) {
  const [copied, setCopied] = useState(false);
  const [mode, setMode] = useState<TrackingMode>(() => inferTrackingMode(snippet, 'direct'));
  const [directBase, setDirectBase] = useState<string | null>(() => {
    const inferredMode = inferTrackingMode(snippet, 'direct');
    return inferredMode === 'direct' ? extractTrackingBase(snippet) : null;
  });
  const [proxyPath, setProxyPath] = useState<string>(() =>
    extractProxyPath(snippet) ?? DEFAULT_FIRST_PARTY_PROXY_PATH,
  );

  useEffect(() => {
    const inferredMode = inferTrackingMode(snippet, 'direct');
    setDirectBase(inferredMode === 'direct' ? extractTrackingBase(snippet) : null);
    setMode(inferredMode);
    setProxyPath(extractProxyPath(snippet) ?? DEFAULT_FIRST_PARTY_PROXY_PATH);
  }, [snippet]);

  useEffect(() => {
    if (directBase || typeof window === 'undefined') {
      return;
    }
    setDirectBase(window.location.origin);
  }, [directBase]);

  const resolvedSnippet = useMemo(() => {
    if (mode === 'first_party') {
      return buildFirstPartyTrackingSnippet(websiteId, proxyPath);
    }
    return buildTrackingSnippet(websiteId, directBase ?? '');
  }, [directBase, mode, proxyPath, websiteId]);

  async function handleCopy() {
    await navigator.clipboard.writeText(resolvedSnippet);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-line bg-canvas">
        <div className="border-b border-line px-4 py-3">
          <p className="text-xs font-medium text-ink">Choose your tracking mode</p>
          <p className="mt-1 text-[11px] text-ink-3">
            Direct analytics origins can be blocked by browser extensions or Cloudflare
            challenges. First-party proxy paths are the safer production choice for public sites.
          </p>
        </div>
        <div className="space-y-3 p-4">
          {MODE_OPTIONS.map((option) => (
            <label
              key={option.id}
              className={`flex cursor-pointer items-start gap-3 rounded-md border px-3 py-3 transition-colors ${
                mode === option.id ? 'border-spark bg-surface-1' : 'border-line bg-surface-1/40'
              }`}
            >
              <input
                type="radio"
                name="tracking-mode"
                value={option.id}
                checked={mode === option.id}
                onChange={() => setMode(option.id)}
                className="mt-0.5 h-4 w-4 accent-[var(--spark)]"
              />
              <span className="space-y-1">
                <span className="block text-xs font-medium text-ink">{option.label}</span>
                <span className="block text-[11px] text-ink-3">{option.hint}</span>
              </span>
            </label>
          ))}

          {mode === 'first_party' && (
            <label className="block">
              <span className="mb-1 block text-xs text-ink-2">Proxy path</span>
              <input
                value={proxyPath}
                onChange={(event) => setProxyPath(normalizeProxyPath(event.target.value))}
                className="w-full rounded-md border border-line bg-canvas px-3 py-2 text-sm text-ink focus:border-spark focus:outline-none focus:ring-2 focus:ring-spark"
              />
              <span className="mt-1 block text-[11px] text-ink-4">
                Route <code className="text-ink-3">{normalizeProxyPath(proxyPath)}/s.js</code> to
                Sparklytics <code className="text-ink-3">/s.js</code> and{' '}
                <code className="text-ink-3">{normalizeProxyPath(proxyPath)}/e</code> to{' '}
                <code className="text-ink-3">/e</code>.
              </span>
            </label>
          )}
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-sm text-ink-2">Tracking snippet</span>
          <Button variant="ghost" size="sm" onClick={handleCopy} className="gap-1 text-xs">
            {copied ? <Check className="h-3 w-3 text-spark" /> : <Copy className="h-3 w-3" />}
            {copied ? 'Copied' : 'Copy'}
          </Button>
        </div>
        <pre className="overflow-x-auto whitespace-pre-wrap break-all rounded-md border border-line bg-canvas p-4 text-xs text-ink-2">
          {resolvedSnippet}
        </pre>
        <p className="text-xs text-ink-3">
          Paste this snippet inside the <code className="text-ink-2">&lt;head&gt;</code> tag of
          your website.
        </p>
      </div>
    </div>
  );
}
