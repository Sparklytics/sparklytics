# Next.js SDK Requirements Specification

**Package:** `@sparklytics/next`
**Language:** TypeScript
**Target:** Next.js 14+ (App Router + Pages Router)
**Bundle Size Target:** <5KB gzipped (core), <8KB with A/B testing
**License:** MIT

## Package Structure

```
packages/next/
├── package.json
├── tsconfig.json
├── tsup.config.ts          # Build config
├── README.md
├── src/
│   ├── index.ts            # Main exports
│   ├── provider.tsx        # SparklyticsProvider (client component)
│   ├── hook.ts             # useSparklytics()
│   ├── experiment.tsx      # useExperiment() - A/B testing (V1.1)
│   ├── event.tsx           # SparklyticsEvent component
│   ├── tracker.ts          # Core tracking logic
│   ├── csp.ts              # CSP header generation helpers
│   ├── consent.ts          # GDPR consent management
│   └── types.ts            # TypeScript definitions
└── dist/                   # Build output (ESM + CJS)
```

## Core Principle: Zero-Config

The entire value proposition of this SDK is that a developer goes from `npm install` to working analytics in under 60 seconds. Every feature must have sensible defaults. Nothing should require configuration to work.

**The whole integration:**

```tsx
// app/layout.tsx - THIS IS IT. NOTHING ELSE.
import { SparklyticsProvider } from '@sparklytics/next'

export default function RootLayout({ children }) {
  return (
    <html>
      <body>
        <SparklyticsProvider websiteId="site_abc123">
          {children}
        </SparklyticsProvider>
      </body>
    </html>
  )
}
```

What happens automatically under the hood:
- Detects App Router vs Pages Router
- Auto-tracks all route changes via `usePathname()` or `Router.events`
- Injects tracking script inline (no external script load = harder for ad blockers)
- Works in Edge Runtime (no Node.js-only APIs)
- TypeScript types included
- SSR-safe (no window access on server)
- React Strict Mode compatible (deduplication built in)

## API Design

### SparklyticsProvider

```tsx
// app/layout.tsx (App Router)
import { SparklyticsProvider } from '@sparklytics/next';

export default function RootLayout({ children }) {
  return (
    <html>
      <body>
        <SparklyticsProvider
          websiteId="site_abc123"
          // All below are OPTIONAL with sensible defaults
          endpoint="https://analytics.example.com"  // default: auto-detect
          autoTrack={true}           // default: true
          respectDNT={false}         // default: false
          trackServerActions={true}  // default: true (V1.1)
          consent="auto"             // default: "auto" (see consent section)
        >
          {children}
        </SparklyticsProvider>
      </body>
    </html>
  );
}
```

```tsx
// pages/_app.tsx (Pages Router) - same API, different file
import { SparklyticsProvider } from '@sparklytics/next';

export default function App({ Component, pageProps }) {
  return (
    <SparklyticsProvider websiteId="site_abc123">
      <Component {...pageProps} />
    </SparklyticsProvider>
  );
}
```

### useSparklytics Hook

```tsx
'use client';
import { useSparklytics } from '@sparklytics/next';

export function SignupButton() {
  const { track, isReady } = useSparklytics();

  return (
    <button onClick={() => track('signup', { plan: 'pro' })}>
      Sign Up
    </button>
  );
}
```

### useExperiment Hook (V1.1 - A/B Testing)

```tsx
'use client';
import { useExperiment } from '@sparklytics/next';

export function PricingPage() {
  const { variant, track: trackConversion } = useExperiment('pricing-test', {
    variants: ['control', 'higher-price'],
    weights: [50, 50],  // default: equal distribution
  });

  return (
    <div>
      <h1>Pricing</h1>
      <Price amount={variant === 'higher-price' ? 99 : 79} />
      <button onClick={() => trackConversion('purchase', { revenue: variant === 'higher-price' ? 99 : 79 })}>
        Buy Now
      </button>
    </div>
  );
}
```

How `useExperiment` works under the hood:
1. On first render: generates deterministic variant assignment from `hash(visitor_id + experiment_id)`
2. Stores variant in memory (no cookies, no localStorage)
3. Sends experiment exposure event to server: `{ type: "experiment", experiment_id, variant }`
4. `trackConversion()` sends conversion event linked to the experiment
5. Same visitor always sees same variant (deterministic hash)
6. Dashboard shows: exposures per variant, conversions, conversion rate, statistical significance

### SparklyticsEvent Component

Declarative event tracking (fires on mount):

```tsx
import { SparklyticsEvent } from '@sparklytics/next';

export function PricingPage() {
  return (
    <>
      <SparklyticsEvent name="pricing_viewed" data={{ source: 'nav' }} />
      <h1>Pricing</h1>
    </>
  );
}
```

## TypeScript Types

```typescript
// types.ts
export interface SparklyticsConfig {
  websiteId: string;
  endpoint?: string;
  autoTrack?: boolean;              // default: true
  respectDNT?: boolean;             // default: false
  trackServerActions?: boolean;     // default: true
  consent?: 'auto' | 'required' | 'granted';  // default: 'auto'
}

export interface TrackFunction {
  (eventName: string, data?: Record<string, string | number | boolean>): void;
}

export interface SparklyticsContext {
  track: TrackFunction;
  isReady: boolean;
}

export interface ExperimentConfig {
  variants: string[];               // at least 2
  weights?: number[];               // percentages, must sum to 100
}

export interface ExperimentResult {
  variant: string;                  // which variant this visitor sees
  track: (conversionName: string, data?: Record<string, string | number | boolean>) => void;
}

export interface SparklyticsProviderProps extends SparklyticsConfig {
  children: React.ReactNode;
}

export interface SparklyticsEventProps {
  name: string;
  data?: Record<string, string | number | boolean>;
}
```

## Advanced Features

### Auto-CSP Configuration

Many Next.js developers struggle with Content Security Policy headers blocking analytics scripts. We provide a helper:

```typescript
// next.config.ts
import { sparklyticsCSP } from '@sparklytics/next/csp';

const nextConfig = {
  headers: async () => [
    {
      source: '/(.*)',
      headers: sparklyticsCSP({
        endpoint: 'https://analytics.example.com',
        // Merges with your existing CSP directives
        existing: "default-src 'self'; script-src 'self'"
      }),
    },
  ],
};
```

This generates the correct `connect-src` and `script-src` directives. No more CSP debugging.

For most users this isn't needed because the SDK inlines the tracking script (no external script-src required) and sends data to the same origin or a configured endpoint.

### Server Actions Tracking (V1.1)

Next.js Server Actions are invisible to client-side analytics. We optionally intercept them:

```typescript
// When trackServerActions={true} (default), the SDK patches
// the form submission handler to track Server Action calls.
// This works by wrapping the action prop on <form> elements.

// Tracked automatically:
// - Form submissions that trigger Server Actions
// - Event name: "server_action"
// - Properties: { action_name, form_id, success: boolean }
```

Implementation: The provider wraps children in a context that monkey-patches `React.useTransition` to detect Server Action calls. This is opt-in via `trackServerActions={true}` and adds ~1KB to bundle.

### GDPR Consent Management

Three modes:

**`consent="auto"` (default):** Since Sparklytics doesn't use cookies or collect PII, no consent is required under GDPR Article 6(1)(f) legitimate interest. Analytics work immediately. This is the same legal basis Plausible and Fathom use.

**`consent="required"`:** Analytics are paused until the user explicitly consents. Provides a `useConsent()` hook:

```tsx
import { useConsent } from '@sparklytics/next';

function CookieBanner() {
  const { grantConsent, revokeConsent, hasConsented } = useConsent();

  if (hasConsented !== null) return null;

  return (
    <div>
      <p>We use privacy-friendly analytics.</p>
      <button onClick={grantConsent}>Accept</button>
      <button onClick={revokeConsent}>Decline</button>
    </div>
  );
}
```

**`consent="granted"`:** Assumes consent already handled externally. Analytics active immediately.

## Implementation Details

### Tracking Logic

```typescript
// tracker.ts - core tracking, no React dependency
class Tracker {
  private endpoint: string;
  private websiteId: string;
  private queue: any[] = [];
  private flushing = false;

  constructor(config: SparklyticsConfig) {
    this.websiteId = config.websiteId;
    this.endpoint = config.endpoint || this.detectEndpoint();
  }

  async trackPageview(url: string, referrer?: string): Promise<void> {
    await this.send({
      type: 'pageview',
      website_id: this.websiteId,
      url,
      referrer: referrer || document.referrer,
      screen: `${window.screen.width}x${window.screen.height}`,
      language: navigator.language,
    });
  }

  async trackEvent(name: string, data?: Record<string, any>): Promise<void> {
    await this.send({
      type: 'event',
      website_id: this.websiteId,
      url: window.location.pathname,
      event_name: name,
      event_data: data,
    });
  }

  async trackExperiment(experimentId: string, variant: string): Promise<void> {
    await this.send({
      type: 'experiment',
      website_id: this.websiteId,
      url: window.location.pathname,
      experiment_id: experimentId,
      variant,
    });
  }

  async trackConversion(experimentId: string, variant: string, name: string, data?: Record<string, any>): Promise<void> {
    await this.send({
      type: 'conversion',
      website_id: this.websiteId,
      url: window.location.pathname,
      experiment_id: experimentId,
      variant,
      event_name: name,
      event_data: data,
    });
  }

  private async send(payload: any): Promise<void> {
    try {
      // Batch: queue events, flush on requestIdleCallback or after 1s
      this.queue.push(payload);
      if (!this.flushing) {
        this.flushing = true;
        const flush = () => {
          const batch = this.queue.splice(0);
          if (batch.length === 0) { this.flushing = false; return; }
          fetch(`${this.endpoint}/api/collect`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(batch.length === 1 ? batch[0] : batch),
            keepalive: true,
          }).catch(() => {}); // Silent failure
          this.flushing = false;
        };
        if ('requestIdleCallback' in window) {
          requestIdleCallback(flush, { timeout: 1000 });
        } else {
          setTimeout(flush, 100);
        }
      }
    } catch {
      // Silent failure - never break the host app
    }
  }

  private detectEndpoint(): string {
    // Default: same origin (self-hosted on same domain)
    if (typeof window !== 'undefined') {
      return window.location.origin;
    }
    return '';
  }
}
```

### Route Change Detection

**App Router (Next.js 14+):**
```typescript
'use client';
import { usePathname, useSearchParams } from 'next/navigation';
import { useEffect, useRef } from 'react';

function RouteTracker({ tracker }: { tracker: Tracker }) {
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const prevPath = useRef<string | null>(null);

  useEffect(() => {
    const url = pathname + (searchParams?.toString() ? `?${searchParams}` : '');

    // Skip duplicate fires (React strict mode)
    if (prevPath.current !== url) {
      tracker.trackPageview(url);
      prevPath.current = url;
    }
  }, [pathname, searchParams, tracker]);

  return null;
}
```

**Pages Router (Next.js <14 or pages/):**
```typescript
import Router from 'next/router';

useEffect(() => {
  // Track initial pageview
  tracker.trackPageview(window.location.pathname + window.location.search);

  const handleRouteChange = (url: string) => {
    tracker.trackPageview(url);
  };

  Router.events.on('routeChangeComplete', handleRouteChange);
  return () => Router.events.off('routeChangeComplete', handleRouteChange);
}, [tracker]);
```

### Experiment Variant Assignment

```typescript
// Deterministic variant assignment - same visitor always sees same variant
function assignVariant(visitorId: string, experimentId: string, variants: string[], weights?: number[]): string {
  // Hash visitor+experiment to get a number 0-99
  const hash = simpleHash(`${visitorId}:${experimentId}`);
  const bucket = hash % 100;

  // Default: equal weights
  const w = weights || variants.map(() => 100 / variants.length);

  let cumulative = 0;
  for (let i = 0; i < variants.length; i++) {
    cumulative += w[i];
    if (bucket < cumulative) return variants[i];
  }
  return variants[variants.length - 1];
}

// Simple non-crypto hash (FNV-1a) - deterministic, fast
function simpleHash(str: string): number {
  let hash = 2166136261;
  for (let i = 0; i < str.length; i++) {
    hash ^= str.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}
```

### Auto-Detection

The SDK automatically detects which router is in use at runtime:

```typescript
function detectRouterType(): 'app' | 'pages' {
  // App Router exports usePathname from next/navigation
  // Pages Router uses next/router
  // We detect by checking which module resolves
  try {
    require.resolve('next/navigation');
    return 'app';
  } catch {
    return 'pages';
  }
}
```

## Build Configuration

```typescript
// tsup.config.ts
export default defineConfig({
  entry: {
    index: 'src/index.ts',
    csp: 'src/csp.ts',       // Separate entry for CSP helpers
  },
  format: ['esm', 'cjs'],
  dts: true,
  external: ['react', 'next'],
  treeshake: true,
  minify: true,
  target: 'es2020',
});
```

## Package.json

```json
{
  "name": "@sparklytics/next",
  "version": "0.1.0",
  "description": "Zero-config analytics SDK for Next.js. Privacy-first, built-in A/B testing.",
  "main": "dist/index.js",
  "module": "dist/index.mjs",
  "types": "dist/index.d.ts",
  "exports": {
    ".": {
      "import": "./dist/index.mjs",
      "require": "./dist/index.js",
      "types": "./dist/index.d.ts"
    },
    "./csp": {
      "import": "./dist/csp.mjs",
      "require": "./dist/csp.js",
      "types": "./dist/csp.d.ts"
    }
  },
  "peerDependencies": {
    "next": ">=14.0.0",
    "react": ">=18.0.0"
  },
  "keywords": ["analytics", "nextjs", "privacy", "ab-testing", "sparklytics"],
  "license": "MIT",
  "repository": "github:sparklytics/sparklytics"
}
```

## Testing

**Unit tests (vitest):**
- Tracker class: payload formatting, silent error handling, event batching
- Router detection logic
- Config defaults
- Experiment variant assignment (deterministic)
- CSP header generation
- Consent state management

**Integration tests:**
- Provider renders without errors
- Route changes trigger pageview
- Custom events fire correctly
- SSR: no window access on server
- Experiment: same visitor always gets same variant
- Consent: events blocked until consent granted (in required mode)

**E2E tests (playwright):**
- Next.js App Router app with SDK installed
- Next.js Pages Router app with SDK installed
- Verify events reach the server
- Verify route change tracking
- Verify experiment tracking roundtrip

## Edge Cases

1. **SSR safety:** All `window`/`document` access guarded with `typeof window !== 'undefined'`
2. **React Strict Mode:** Deduplication via `useRef` prevents double pageview on mount
3. **Fast navigation:** If user navigates faster than fetch completes, `keepalive` ensures events are sent
4. **Blocked by ad blockers:** Script inlined in the bundle (not loaded from external URL), harder to block. Endpoint path `/api/collect` can be proxied through Next.js API route to bypass blockers completely.
5. **No JavaScript:** Graceful degradation - analytics just don't work, no errors
6. **Next.js middleware:** SDK works regardless of middleware configuration
7. **ISR/SSG pages:** Tracking happens client-side only, works with all rendering modes
8. **Turbopack:** Compatible with Next.js Turbopack development mode
9. **Multi-zone apps:** Each zone can have its own SparklyticsProvider with different websiteId
10. **Ad blocker proxy bypass:** Users can create a Next.js API route that proxies to the analytics endpoint, making tracking requests indistinguishable from first-party API calls
