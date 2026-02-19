# Sparklytics Design System

## Direction

**"Precision instrument"**

The developer opens this dashboard the same way they open a terminal — expecting information density without decoration. Dark by default, legible, confident. Every element earns its place. Structure comes from typographic hierarchy, not colored surfaces. Spark green appears only where something is live or growing.

Not Vercel (too polished, too much whitespace). Not Plausible (too minimal, no personality). Between a terminal and a dashboard — precise, slightly opinionated, proud of its data.

---

## Intent

**Who:** A developer checking their site's analytics — running a personal site, SaaS, or small business. Technical but not a data scientist. Open it at inconsistent times: quick mobile check, deeper Friday review. Self-hosted by choice — they've opted out of surveillance analytics.

**What they must do:** Understand if the site is growing or declining. Find what content or source is driving (or killing) traffic. Spot breakage in real-time. Make decisions about what to build or write next.

**How it should feel:** Calm like a monitoring terminal. The signal is clear. Nothing competes for attention that doesn't deserve it.

---

## Color Palette

### Primitives

```css
/* Dark mode (default) */
--canvas:       #0F0F0F    /* Page background */
--surface-1:    #1E1E1E    /* Cards, panels */
--surface-2:    #2D2D2D    /* Hover, active surface */
--surface-input: #111111   /* Form inputs (inset feel) */

/* Text */
--ink:          #F8F8F8    /* Primary text */
--ink-2:        #94A3B8    /* Secondary — labels, supporting */
--ink-3:        #64748B    /* Tertiary — metadata, timestamps */
--ink-4:        #475569    /* Muted — disabled, placeholder */

/* Borders */
--line:         #2A2A2A    /* Standard separation (whisper) */
--line-2:       #1E1E1E    /* Softer — internal grouping */
--line-3:       #475569    /* Emphasis — focused inputs */

/* Brand */
--spark:        #00D084    /* Primary accent — live, growing, active */
--spark-dim:    #00B86D    /* Pressed state */
--spark-subtle: rgba(0, 208, 132, 0.08)  /* Background tint */

/* Semantic */
--up:           #10B981    /* Success / positive trend */
--down:         #EF4444    /* Error / negative trend */
--warn:         #F59E0B    /* Warning */
--neutral:      #3B82F6    /* Info / second data series */
```

### Chart Colors (ordered by priority)
```
1. #00D084  spark green      — primary series (visitors)
2. #3B82F6  electric blue    — secondary series (pageviews)
3. #F59E0B  amber            — tertiary
4. #EC4899  rose             — quaternary
5. #8B5CF6  violet           — fifth
6. #06B6D4  cyan             — sixth
```

### Token Naming Rationale

`--ink` and `--canvas` evoke writing on a surface. `--line` for borders. `--spark` for the brand. Token names should be readable aloud and identify the product's character.

---

## Typography

```
Body / UI:    Inter (400, 500, 600)
Numbers:      IBM Plex Mono — ALL analytics numbers use this, not just code
Code:         IBM Plex Mono

Heading letter-spacing: -0.5px (tighter)
Number tracking:        tabular-nums (CSS font-variant-numeric: tabular-nums)
```

**The rule:** If it's a metric, it's monospace. If it's a label, it's Inter. This creates instant visual distinction between "data" and "context."

---

## Depth Strategy

**Borders-only.** No box shadows except minimal focus rings.

Dark surfaces don't benefit from shadows (invisible against dark backgrounds). Use border-only to define elevation. Surface color shifts handle hierarchy.

```
Page background    →  --canvas (#0F0F0F)
Card               →  --surface-1 (#1E1E1E) + 1px --line border
Dropdown / popover →  --surface-2 (#2D2D2D) + 1px --line-3 border
Input              →  --surface-input (#111111) + 1px --line border
Focus ring         →  2px --spark, offset 2px
```

---

## Spacing

Base unit: **4px**

```
2px   — icon gap, badge internal
4px   — tight inline spacing
8px   — component internal (button padding-y, icon-label gap)
12px  — component internal (button padding-x)
16px  — card padding (small), section grouping
24px  — card padding (large), content padding mobile
32px  — section break
48px  — major section separation
```

---

## Border Radius

```
2px  — badges, status pills
4px  — buttons, small controls
6px  — inputs, selects
8px  — cards, panels
12px — modals, drawers
```

Sharp-leaning. Analytics is precise, not rounded. Nothing above 12px.

---

## Layout

```
Sidebar:        200px wide
                Same --canvas background as content
                1px --line border on right (no color block)
                Identity through typography, not a colored panel

Content:        padding: 24px (desktop), 16px (mobile)
Header:         height: 56px, border-bottom: 1px --line

Stat card grid: 1 col → 2 col (sm) → 5 col (lg)
Card padding:   16px (stat cards), 24px (chart cards)
```

---

## Component Patterns

### Stat Card

Numbers lead. The metric value is the primary element — large, monospace, tabular. The label sits below in Inter at secondary weight. Trend delta is a small badge (colored by direction). Sparkline sits at the bottom as a 30px strip.

Do NOT: icon-left layout, equal visual weight between label and number.

```
Structure:
  [metric label — Inter 12px secondary]
  [value — IBM Plex Mono 28px primary, tabular]
  [trend badge — delta% with ↑↓ indicator]
  [sparkline strip — 30px, no axes]
```

### Navigation / Sidebar

Same canvas background. Links in Inter 500 at secondary color, active link in primary color with spark green left indicator (2px bar). No colored backgrounds on active items — only the bar and text color change.

```
Nav item: py-2 px-3, rounded-md on hover (--surface-1)
Active:   left 2px border --spark, --ink color
Icon:     w-5 h-5 Lucide, same color as text
```

### Charts

```
CartesianGrid:  stroke --line, strokeDasharray 3 3
Axis:           stroke --ink-3, no tick lines
Tooltip:        bg --surface-2, border --line-3, values in IBM Plex Mono
Legend:         NOT rendered in chart — color dots + labels in section header instead
```

### Realtime Indicator

Pulsing dot — `--spark` color, `animate-pulse` (or custom `pulse-spark` keyframe). Used only for live visitor count. Not used decoratively elsewhere.

### Tables (breakdown data)

Minimal borders — row separator only (`border-b --line`). No column borders. No background alternation. Hover: `--surface-1` background. Values right-aligned, monospace. Bar behind percentage column (subtle `--spark-subtle` fill).

---

## Rejected Defaults

| Default | What We Do Instead |
|---------|-------------------|
| Icon-left + big-number + small-label stat cards | Typographic readouts — number leads, label is secondary |
| Colored sidebar with logo at top | Same background as content, border-only separation |
| Recharts LineChart with legend inside chart | Muted grid, color dots in section header, no in-chart legend |
| box-shadows for card depth | Border-only depth strategy |
| All text in Inter | Analytics numbers in IBM Plex Mono |

---

## States (required for all interactive elements)

```
Default   →  standard appearance
Hover     →  --surface-1 or --surface-2 background shift
Active    →  slight scale(0.98) or darker surface
Focus     →  2px --spark ring, offset 2px
Disabled  →  opacity-40, cursor-not-allowed
Loading   →  Skeleton pulse (--surface-1 → --surface-2) or Loader2 spin in --spark
Empty     →  Centered illustration-free state with tracking snippet
Error     →  --down color, border-color shift, icon + message
```

---

## Icons

Lucide React exclusively.
- Nav: `w-5 h-5` (20px)
- Buttons / inline: `w-4 h-4` (16px)
- Loading: `Loader2` with `animate-spin text-spark`

No decorative icons. Every icon must clarify action or state.

---

## Animation

Tailwind-only. No Framer Motion.

```
Micro-interactions:  100-150ms ease-out
UI transitions:      200-300ms ease-out
Chart entry:         600ms (Recharts built-in isAnimationActive)
Live pulse:          pulse-spark keyframe, 2s infinite
```

No spring or bounce. Professional interfaces don't bounce.

---

## Performance

Bundle target: `<250KB` gzipped
TTI target: `<2s`
Lighthouse: `>90`

---

## Tech Stack

```
Framework:       Next 16
Styling:         TailwindCSS + shadcn/ui (do not hand-edit ui/ components)
State:           TanStack Query (server), Zustand (UI)
Charts:          Recharts
Icons:           Lucide React
Dark mode:       class-based, localStorage key: sparklytics-theme
Default mode:    dark
```
