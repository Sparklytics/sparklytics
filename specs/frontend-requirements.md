# Frontend Requirements Specification

**Component:** Sparklytics dashboard (`dashboard/`)  
**Framework:** Next.js 15 App Router + React 19  
**Styling:** TailwindCSS 3 + shadcn/ui primitives  
**State:** TanStack Query + local UI state helpers  
**Status:** Aligned with current public repo state (2026-02-27)

## Project Structure (Current)

```text
dashboard/
├── app/
│   ├── layout.tsx
│   ├── page.tsx
│   ├── (auth)/login/page.tsx
│   ├── setup/page.tsx
│   ├── onboarding/page.tsx
│   ├── settings/page.tsx
│   ├── share/page.tsx
│   └── dashboard/
│       ├── page.tsx
│       └── DashboardClient.tsx
├── components/
├── hooks/
│   ├── useAuth.ts
│   ├── useStats.ts
│   ├── usePageviews.ts
│   ├── useMetrics.ts
│   ├── useRealtime.ts
│   ├── useFunnels.ts / useFunnelResults.ts
│   ├── useJourney.ts / useRetention.ts
│   ├── useReports.ts / useReportRun.ts
│   ├── useCampaignLinks.ts / useTrackingPixels.ts
│   ├── useNotifications.ts / useBots.ts
│   └── ...
├── lib/
│   ├── api.ts
│   ├── config.ts
│   ├── runtime.ts
│   └── utils.ts
└── next.config.ts
```

## Runtime and Build Behavior

`next.config.ts` requirements:

- Development: no static export, with `/api/:path*` rewrite to `http://localhost:3000`
- Production build: static export enabled (`output: "export"`)
- `trailingSlash: true`
- `images.unoptimized: true` (for static export compatibility)

Build output is `dashboard/out/`, embedded into the Rust binary.

## Auth UX Contracts

- Dashboard checks `GET /api/auth/status` via `useAuth`
- Auth modes:
  - `local`: supports first-run setup + login
  - `password`: login flow
  - `none`: auth routes are not registered; frontend handles 404 from auth status gracefully
- Session auth is cookie-based for dashboard APIs

## Functional Coverage

Dashboard must provide and keep parity with current server endpoints:

- Core analytics: stats, pageviews, metrics, realtime
- Events and sessions explorers
- Goals + attribution/revenue summaries
- Funnels + journey + retention
- Reports, subscriptions, alerts, notification history
- Campaign links + tracking pixels
- Bot controls/report/recompute status
- Share links and export entry points

## UI and Responsiveness

- Must work on mobile and desktop breakpoints
- Must follow design system tokens from docs (`docs/14-BRAND-STYLE-GUIDE.md`)
- Numeric KPIs should use tabular numeral styling where applicable
- No dependency on Node runtime in production deployment (static assets only)

## Verification Gate

Required after frontend changes:

```bash
cd dashboard
npm run type-check
npm run lint
npm run build
```

If UI components/layout changed, run visual checks in desktop and mobile viewports before merge.
