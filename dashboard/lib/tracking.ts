export type TrackingMode = 'direct' | 'first_party';

export const DEFAULT_FIRST_PARTY_PROXY_PATH = '/_sl';

function trimTrailingSlashes(value: string): string {
  return value.replace(/\/+$/, '');
}

export function normalizeTrackingBase(base: string): string {
  const trimmed = trimTrailingSlashes(base.trim());
  return trimmed || '';
}

export function normalizeProxyPath(path: string): string {
  const trimmed = trimTrailingSlashes(path.trim());
  if (!trimmed) {
    return DEFAULT_FIRST_PARTY_PROXY_PATH;
  }

  return trimmed.startsWith('/') ? trimmed : `/${trimmed}`;
}

export function buildTrackingSnippet(websiteId: string, trackingBase: string): string {
  const normalizedBase = normalizeTrackingBase(trackingBase);
  return `<script defer src="${normalizedBase}/s.js" data-website-id="${websiteId}"></script>`;
}

export function buildFirstPartyTrackingSnippet(
  websiteId: string,
  proxyPath = DEFAULT_FIRST_PARTY_PROXY_PATH,
): string {
  const normalizedPath = normalizeProxyPath(proxyPath);
  return `<script defer src="${normalizedPath}/s.js" data-website-id="${websiteId}"></script>`;
}

export function extractTrackingScriptSrc(snippet?: string): string | null {
  if (!snippet) {
    return null;
  }

  const match = snippet.match(/src="([^"]+)"/i);
  return match?.[1] ?? null;
}

export function inferTrackingMode(
  snippet: string | undefined,
  fallback: TrackingMode = 'direct',
): TrackingMode {
  const src = extractTrackingScriptSrc(snippet);
  if (!src) {
    return fallback;
  }

  try {
    const url = new URL(src, 'https://sparklytics.invalid');
    return trimTrailingSlashes(url.pathname) === '/s.js' ? 'direct' : 'first_party';
  } catch {
    return fallback;
  }
}

export function extractTrackingBase(snippet?: string): string | null {
  const src = extractTrackingScriptSrc(snippet);
  if (!src) {
    return null;
  }

  if (src.startsWith('/')) {
    return null;
  }

  try {
    return trimTrailingSlashes(src.slice(0, -'/s.js'.length));
  } catch {
    return null;
  }
}

export function extractProxyPath(snippet?: string): string | null {
  const src = extractTrackingScriptSrc(snippet);
  if (!src) {
    return null;
  }

  try {
    const url = new URL(src, 'https://sparklytics.invalid');
    const pathname = trimTrailingSlashes(url.pathname);
    if (!pathname.endsWith('/s.js')) {
      return null;
    }
    const basePath = pathname.slice(0, -'/s.js'.length);
    return normalizeProxyPath(basePath || DEFAULT_FIRST_PARTY_PROXY_PATH);
  } catch {
    return null;
  }
}
