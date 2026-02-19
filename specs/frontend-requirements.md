# Frontend Requirements Specification

**Component:** Sparklytics Dashboard (SPA)
**Framework:** React 18+ with Next.js 16 (App Router, `output: 'export'`)
**Styling:** TailwindCSS 3+
**Charts:** Recharts
**State:** TanStack Query (server) + zustand (UI)
**Language:** TypeScript (strict mode)

## Project Structure

```
dashboard/
├── next.config.ts            # output: 'export'; /api rewrite → localhost:3000 in dev
├── tailwind.config.js
├── tsconfig.json
├── package.json
├── app/                      # Next.js App Router
│   ├── layout.tsx            # Root layout (QueryClientProvider, fonts, dark class)
│   ├── page.tsx              # Root → redirect to /dashboard
│   ├── globals.css
│   ├── (auth)/
│   │   └── login/
│   │       └── page.tsx      # Login page (client component)
│   ├── setup/
│   │   └── page.tsx          # First-run setup wizard (client component)
│   └── dashboard/
│       └── [websiteId]/
│           └── page.tsx      # Main analytics view (client component, websiteId via useParams())
├── components/
│   ├── layout/
│   │   ├── AppShell.tsx      # Sidebar + header shell
│   │   ├── Sidebar.tsx       # Navigation (includes WebsitePicker)
│   │   ├── Header.tsx        # Date range picker, filter bar, logout
│   │   └── WebsitePicker.tsx # Dropdown: website selector; shows "Add your first website →" if empty array returned
│   ├── dashboard/
│   │   ├── StatsRow.tsx      # 5 stat cards: Pageviews, Visitors, Sessions, Bounce Rate, Avg Duration
│   │   ├── StatCard.tsx      # KPI with trend % and sparkline (sparkline: number[] from pageviews API)
│   │   ├── PageviewsChart.tsx # Recharts LineChart, dual series
│   │   ├── DataTable.tsx     # Generic sortable table
│   │   ├── RealtimePanel.tsx # Active visitors + recent events
│   │   └── EmptyState.tsx    # No-data state with tracking snippet
│   ├── filters/
│   │   ├── DateRangePicker.tsx  # Presets: Last 7 days, Last 30 days, Last 90 days, Custom; URL params: ?start=&end=
│   │   ├── FilterBar.tsx
│   │   └── FilterChip.tsx
│   └── ui/                   # shadcn/ui (auto-generated, don't hand-edit)
├── hooks/
│   ├── useAuth.ts            # GET /api/auth/status → redirect logic. If 404 → treat as authenticated (none mode)
│   ├── useFilters.ts         # zustand store: { dateRange: { start_date, end_date }, filters: Record<string,string> }
│   │                         # URL params: ?start=&end= (mapped to start_date/end_date when calling API)
│   │                         # Filters: ?filter_page=&filter_country= etc. (AND-ed together)
│   ├── useStats.ts
│   ├── usePageviews.ts
│   ├── useMetrics.ts
│   └── useRealtime.ts
├── lib/
│   ├── api.ts                # fetch wrapper with base URL + auth cookie
│   ├── config.ts             # NEXT_PUBLIC_* env var exports (IS_CLOUD, etc.)
│   └── utils.ts              # cn(), formatNumber(), formatDuration()
└── public/
    └── favicon.svg
```

## Next.js Configuration

```typescript
// next.config.ts
import type { NextConfig } from 'next';

const nextConfig: NextConfig = {
  output: 'export',
  trailingSlash: true,         // generates out/dashboard/[id]/index.html per route
  images: { unoptimized: true }, // required for static export
  async rewrites() {           // dev-only API proxy (ignored in static export build)
    return [
      {
        source: '/api/:path*',
        destination: 'http://localhost:3000/api/:path*',
      },
    ];
  },
};

export default nextConfig;
```

## Page Specifications

### Dashboard Page (Main View)

**Layout:**
```
┌─────────────────────────────────────────────────┐
│ [Website Selector v]  [Date Range: Last 7 Days] │
│ [Active Filters: Country: US x]                 │
├─────────────────────────────────────────────────┤
│ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌─────┐│
│ │Views  │ │Visitors│ │Sessions│ │Bounce │ │Avg  ││
│ │12,450 │ │ 3,200 │ │ 4,100 │ │ 42%   │ │3:05 ││
│ │ +11%  │ │ +10%  │ │ +11%  │ │  -7%  │ │+9%  ││
│ └───────┘ └───────┘ └───────┘ └───────┘ └─────┘│
├─────────────────────────────────────────────────┤
│ ┌───────────────────────────────────────────┐   │
│ │         Pageviews Line Chart              │   │
│ │    ─────────────────────────────          │   │
│ │   /                                       │   │
│ │──/                                        │   │
│ └───────────────────────────────────────────┘   │
├────────────────────┬────────────────────────────┤
│  Top Pages         │  Top Referrers             │
│  / ........... 4.5K│  google.com ......... 1.2K │
│  /blog ....... 2.3K│  twitter.com ......... 450 │
│  /pricing .... 800 │  (direct) ........... 800  │
│  /docs ....... 650 │  github.com .......... 200 │
├────────────────────┼────────────────────────────┤
│  Countries         │  Devices                   │
│  US ......... 1.5K │  Desktop ......... 65%     │
│  DE ........... 400│  Mobile .......... 30%     │
│  PL ........... 350│  Tablet ........... 5%     │
└────────────────────┴────────────────────────────┘
```

### Stat Cards

Each card shows:
- Label (e.g., "Visitors")
- Current value (formatted: 3,200 or 3.2K for >9999)
- Change percentage vs previous period (green up, red down)

### Line Chart

- Recharts `<LineChart>` with `<Line>` for pageviews
- Optional second line for visitors (togglable)
- Tooltip on hover showing exact values
- Responsive (fills container width)
- Granularity: auto based on date range

### Data Tables

- Sortable by clicking column headers
- Show top 10 by default, "Show more" button
- Click a row to apply it as a filter
- Compact design (no borders, just alternating rows)

## Responsive Design

**Breakpoints:**
- Mobile: <640px (single column, cards stack vertically)
- Tablet: 640-1024px (2-column layout for tables)
- Desktop: >1024px (full layout as shown above)

**Mobile adaptations:**
- Stats row becomes horizontally scrollable
- Tables show 2 columns max (label + value)
- Chart reduces to simple sparkline
- Date picker becomes a modal

## API Integration

Using TanStack Query for all server state:

```typescript
// hooks/useStats.ts
export function useStats(websiteId: string) {
  const { dateRange, filters } = useFilters();

  return useQuery({
    queryKey: ['stats', websiteId, dateRange, filters],
    queryFn: () => api.getStats(websiteId, { ...dateRange, ...filters }),
    staleTime: 60_000,      // 1 minute
    refetchInterval: 60_000, // Auto-refresh every minute
  });
}
```

**Cache strategy:**
- Stats: stale after 1 minute, refetch every minute
- Pageviews: stale after 1 minute
- Metrics: stale after 1 minute
- Realtime: refetch every 30 seconds (polling)

## Design System

**Color palette:**
- Primary: #00D084 (`spark` — electric green)
- Background (dark default): #0F0F0F (`--canvas`)
- Surface: #1E1E1E (`--surface-1`), #2D2D2D (`--surface-2`)
- Text: #F8F8F8 (`--ink`), #94A3B8 (`--ink-2`)
- Success/up: #10B981, Danger/down: #EF4444, Warn: #F59E0B
- Chart colors: `chart-0` through `chart-5` (defined in tailwind.config.js)
- → Full token set in `docs/14-BRAND-STYLE-GUIDE.md` and `app/globals.css`

**Typography:**
- Body font: Inter (400/500/600/700)
- Mono font: IBM Plex Mono (400/500/600) — used for all numbers and code
- Headings: font-semibold
- Body: font-normal
- Numbers: `font-mono tabular-nums` (IBM Plex Mono, not generic monospace)

**Spacing:**
- Consistent use of Tailwind spacing scale
- Cards: p-4 or p-6
- Gaps between cards: gap-4
- Page padding: px-4 md:px-6 lg:px-8

## Build & Embedding

Dashboard builds to static files via Next.js:
```bash
cd dashboard && npm run build
# Runs: next build (with output: 'export' in next.config.ts)
# Output: dashboard/out/
```

These files are embedded into the Rust binary at compile time:
```rust
use include_dir::{include_dir, Dir};
static DASHBOARD: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../dashboard/out");
```

Next.js static export creates one `index.html` per route directory. Axum serves them with a multi-level fallback:
```rust
async fn serve_dashboard(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    // Try exact file first (JS chunks, CSS, images, favicon)
    if let Some(file) = DASHBOARD.get_file(path) {
        return serve_file(file);
    }
    // Try path/index.html (Next.js generates one per route)
    let index_path = format!("{}/index.html", path.trim_end_matches('/'));
    if let Some(file) = DASHBOARD.get_file(&index_path) {
        return serve_file(file);
    }
    // Fallback: root index.html (client handles 404)
    serve_file(DASHBOARD.get_file("index.html").unwrap())
}
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Dashboard bundle (gzipped) | <250KB |
| Time to Interactive | <2s (cold) |
| Subsequent navigation | <500ms |
| Chart rendering | <100ms |
| Lighthouse Performance | >90 |

## Accessibility

- All interactive elements keyboard-navigable
- ARIA labels on charts and tables
- Color contrast ratio > 4.5:1
- Focus indicators visible
- Screen reader support for stat cards
