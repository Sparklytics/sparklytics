# Frontend Requirements Specification

**Component:** Sparklytics Dashboard (SPA)
**Framework:** React 18+ with Vite
**Styling:** TailwindCSS 3+
**Charts:** Recharts
**State:** TanStack Query (server) + zustand (UI)
**Language:** TypeScript (strict mode)

## Project Structure

```
dashboard/
├── index.html
├── vite.config.ts
├── tailwind.config.js
├── tsconfig.json
├── package.json
├── src/
│   ├── main.tsx              # Entry point
│   ├── App.tsx               # Root component, routing
│   ├── api/                  # API client
│   │   ├── client.ts         # Fetch wrapper with auth
│   │   ├── types.ts          # API response types
│   │   └── hooks.ts          # TanStack Query hooks
│   ├── components/
│   │   ├── layout/
│   │   │   ├── Header.tsx    # Website selector, date picker
│   │   │   ├── Sidebar.tsx   # Navigation (if needed)
│   │   │   └── Layout.tsx    # Page wrapper
│   │   ├── charts/
│   │   │   ├── LineChart.tsx  # Pageviews over time
│   │   │   ├── BarChart.tsx   # Breakdown charts
│   │   │   └── PieChart.tsx   # Device/browser breakdown
│   │   ├── tables/
│   │   │   ├── DataTable.tsx  # Generic sortable table
│   │   │   ├── PagesTable.tsx
│   │   │   ├── ReferrersTable.tsx
│   │   │   └── CountriesTable.tsx
│   │   ├── stats/
│   │   │   ├── StatCard.tsx   # Summary stat with change %
│   │   │   └── StatsRow.tsx   # Row of 4-5 stat cards
│   │   ├── filters/
│   │   │   ├── DateRangePicker.tsx
│   │   │   ├── FilterBar.tsx
│   │   │   └── FilterChip.tsx
│   │   └── ui/
│   │       ├── Button.tsx
│   │       ├── Input.tsx
│   │       ├── Modal.tsx
│   │       ├── Dropdown.tsx
│   │       ├── Tabs.tsx
│   │       └── Loading.tsx
│   ├── pages/
│   │   ├── Dashboard.tsx      # Main analytics view
│   │   ├── Realtime.tsx       # Real-time visitors
│   │   ├── Settings.tsx       # Website management
│   │   ├── Login.tsx          # Auth (cloud only)
│   │   ├── Register.tsx       # Auth (cloud only)
│   │   └── Account.tsx        # Account settings (cloud only)
│   ├── stores/
│   │   ├── dateRange.ts       # Date range state (zustand)
│   │   ├── filters.ts         # Active filters state
│   │   └── website.ts         # Selected website
│   ├── utils/
│   │   ├── format.ts          # Number/date formatting
│   │   ├── countries.ts       # Country code -> name + flag
│   │   └── colors.ts          # Chart color palette
│   └── hooks/
│       ├── useStats.ts
│       ├── usePageviews.ts
│       ├── useMetrics.ts
│       └── useRealtime.ts
└── public/
    └── favicon.svg
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
- Realtime: refetch every 5 seconds (polling)

## Design System

**Color palette:**
- Primary: #6366f1 (indigo-500)
- Background: #ffffff (light) / #0f172a (dark future)
- Text: #1e293b (slate-800)
- Muted: #94a3b8 (slate-400)
- Success: #22c55e (green-500)
- Danger: #ef4444 (red-500)
- Chart colors: ['#6366f1', '#8b5cf6', '#a78bfa', '#c4b5fd']

**Typography:**
- Font: Inter (system font stack fallback)
- Headings: font-semibold
- Body: font-normal
- Numbers: tabular-nums (monospace for alignment)

**Spacing:**
- Consistent use of Tailwind spacing scale
- Cards: p-4 or p-6
- Gaps between cards: gap-4
- Page padding: px-4 md:px-6 lg:px-8

## Build & Embedding

Dashboard builds to static files via Vite:
```bash
cd dashboard && npm run build
# Output: dashboard/dist/
```

These files are embedded into the Rust binary at compile time:
```rust
use include_dir::{include_dir, Dir};
static DASHBOARD: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../dashboard/dist");
```

Axum serves them as a fallback route (SPA routing):
```rust
async fn serve_dashboard(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    match DASHBOARD.get_file(path) {
        Some(file) => { /* serve file with correct content-type */ },
        None => { /* serve index.html for SPA routing */ },
    }
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
