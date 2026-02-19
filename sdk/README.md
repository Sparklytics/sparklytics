# @sparklytics/next

Official Next.js SDK for [Sparklytics](https://sparklytics.dev) — privacy-first, self-hostable analytics.

## Installation

```bash
npm install @sparklytics/next
```

## Quick Start

### App Router (`app/layout.tsx`)

```tsx
import { SparklyticsProvider } from '@sparklytics/next'

export default function RootLayout({ children }: { children: React.ReactNode }) {
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

### Pages Router (`pages/_app.tsx`)

```tsx
import type { AppProps } from 'next/app'
import { SparklyticsProvider } from '@sparklytics/next'

export default function App({ Component, pageProps }: AppProps) {
  return (
    <SparklyticsProvider websiteId="site_abc123">
      <Component {...pageProps} />
    </SparklyticsProvider>
  )
}
```

That's it. Pageviews are tracked automatically on every route change.

## Custom Events

Use the `useSparklytics()` hook to track custom events:

```tsx
'use client'

import { useSparklytics } from '@sparklytics/next'

export function SignupButton() {
  const { track } = useSparklytics()

  return (
    <button onClick={() => track('signup_click', { plan: 'pro' })}>
      Get started
    </button>
  )
}
```

## Declarative Click Tracking

Wrap any element with `<SparklyticsEvent>` to track clicks without writing handlers:

```tsx
import { SparklyticsEvent } from '@sparklytics/next'

<SparklyticsEvent name="hero_cta" data={{ variant: 'A' }}>
  <button>Start free trial</button>
</SparklyticsEvent>
```

## Props Reference

### `<SparklyticsProvider>`

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `websiteId` | `string` | **required** | Website UUID from your Sparklytics dashboard |
| `endpoint` | `string` | `''` | Base URL of your Sparklytics server (e.g. `https://analytics.example.com`). SDK appends `/api/collect`. |
| `respectDnt` | `boolean` | `true` | Respect `navigator.doNotTrack` and `navigator.globalPrivacyControl` |
| `disabled` | `boolean` | `false` | Set `true` to disable tracking (e.g. in dev/staging) |
| `nonce` | `string` | — | CSP nonce for inline scripts |

> **Note:** `endpoint` accepts the **base URL only**. The SDK appends `/api/collect` automatically. Passing the full collect URL will result in a doubled path (`/api/collect/api/collect`).

## Self-Hosted Configuration

When using this SDK with a self-hosted Sparklytics server, set the `SPARKLYTICS_CORS_ORIGINS` environment variable on your **server** to include your website's domain:

```bash
SPARKLYTICS_CORS_ORIGINS=https://yourdomain.com,https://www.yourdomain.com
```

Then configure the `endpoint` prop to point to your server:

```tsx
<SparklyticsProvider
  websiteId="site_abc123"
  endpoint="https://analytics.yourdomain.com"
>
```

## Local Development with `npm link`

To test the SDK against a local Next.js app without publishing:

```bash
# In the SDK directory — build and link
cd sdk
npm run build
npm link

# In your Next.js app directory
cd ../my-nextjs-app
npm link @sparklytics/next

# Now import works as if installed from npm
```

To unlink:

```bash
# In your Next.js app directory
npm unlink @sparklytics/next

# In the SDK directory
npm unlink
```

After any SDK changes, run `npm run build` in the SDK directory again. Use `npm run dev` in the SDK directory to rebuild automatically on file changes.

## Privacy

- **DNT / GPC**: When `respectDnt={true}` (default), the SDK checks `navigator.doNotTrack === '1'` and `navigator.globalPrivacyControl === true`. If either is set, no events are sent.
- **No cookies**: The SDK does not set any cookies.
- **Batching**: Events are queued locally and flushed in batches (max 10 events, or after 500ms), reducing network requests.

## Edge Runtime Compatibility

This package does not use `process`, `fs`, `net`, or any other Node.js-only APIs. It is safe to import in any context including Edge Runtime middleware (though the `useSparklytics()` hook must still be used in Client Components only).

## Bundle Size

The gzipped bundle is under 5KB.
