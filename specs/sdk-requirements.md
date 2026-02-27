# Next.js SDK Requirements Specification

**Package:** `@sparklytics/next`  
**Repository:** `https://github.com/Sparklytics/sparklytics-next.git`  
**Location on disk (nested checkout):** `sdk/next/`  
**Status:** Aligned with current SDK code (2026-02-27)

## Package Surface

`package.json` exports currently include only:

- `@sparklytics/next` (client/runtime React API)
- `@sparklytics/next/server` (server-side helpers)

No `@sparklytics/next/csp` export is currently provided.

## Source Structure (Current)

```text
sdk/next/
├── src/
│   ├── index.ts   # provider, hooks, identify/reset, tracked components
│   └── server.ts  # server-side track helpers + wrappers
├── tests/
├── README.md
└── package.json
```

## Client API Requirements (`@sparklytics/next`)

Core API:

- `SparklyticsProvider`
- `useSparklytics()`
- `identify(visitorId: string)`
- `reset()`
- `usePageview(options?)`
- `TrackedLink`
- `Track`

Provider configuration supports:

- `websiteId` (or `NEXT_PUBLIC_SPARKLYTICS_WEBSITE_ID`)
- `host` (or `NEXT_PUBLIC_SPARKLYTICS_HOST`)
- `respectDnt` (default true)
- `disabled`
- auto-instrumentation toggles for links/scroll depth/forms

Tracking behavior:

- Auto pageview on route changes
- Custom events via `track(event, data?)`
- Optional `visitor_id` override from `identify()` (stored in `localStorage` key `sparklytics_visitor_id`)

## Server API Requirements (`@sparklytics/next/server`)

Must provide:

- `trackServerPageview(...)`
- `trackServerEvent(...)`
- `createServerClient(...)`
- `withAnalytics(...)`
- `trackServerMiddleware(...)`

Server helpers must be safe in Route Handlers, Server Actions, and middleware contexts.

## Compatibility

- Peer deps: Next.js `>=13`, React `>=18`
- Works with App Router and Pages Router integrations
- TypeScript declarations must be generated with the package build

## Non-Goals (Current State)

These are not part of the current published API surface:

- `useExperiment`
- CSP helper export path
- built-in consent manager API
- server-action monkeypatch tracking

If introduced later, they must be documented as new versioned capabilities.

## Build and Test

```bash
cd sdk/next
npm run build
npm run test
```
