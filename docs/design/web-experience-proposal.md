# ViperTrade — Operator Console: Web Experience Proposal

> **Status:** Proposal / RFC · **Date:** 2026-06-09 · **Scope:** `services/web`
> **One-line:** Evolve the dashboard from a single strong screen into a coherent
> **operator console** — a mission-control surface for *auditable, evidence-driven*
> algorithmic trading.

---

## 1. Executive summary

The current web app has an excellent `dashboard` and a rich `analysis` page, but
the experience stops there: **3 of 5 nav destinations are "coming soon" stubs**,
there is **dead/duplicated header code**, and the **design tokens have drifted**
(two CSS-variable systems; the configured font isn't the rendered font).

This proposal reframes the product around a single idea — **the Operator Console** —
and delivers a concrete plan: a tightened information architecture, a consolidated
design language, signature components purpose-built for a trading operator, and a
phased roadmap that ships value every week. Nothing here throws away the strong
work already done (Strategy Cockpit, WebSocket live push, Architecture Flow); it
*completes the frame* around it.

---

## 2. Where we are today (grounded review)

| Surface | State | Verdict |
|---|---|---|
| `/dashboard` | Portfolio KPIs, Strategy Cockpit (consensus + RSI/%B gauges), Open Positions, Closed Trades, Architecture Flow | **Strong** — keep & elevate |
| `/analysis` | Overview, Focus, Watchlist, Performance Snapshot | **Strong** — but lives as a sibling route, header wired by hand |
| `/dashboard/trades` | "Trade history page - coming soon" | **Dead end** |
| `/dashboard/positions` | "Positions page - coming soon" | **Dead end** |
| `/dashboard/settings` | "Settings page - coming soon" | **Dead end** |
| `components/layout/Header.tsx` | Not imported anywhere; nav points to non-existent `/trades`, `/positions`, `/settings` | **Delete** |
| Tokens | `tailwind.config` → `Inter`/`JetBrains Mono`; `globals.css` base → `Space Grotesk`; two var sets (`--background` HSL vs legacy `--bg/--panel/--accent-old`) | **Consolidate** |

**The core problem isn't quality — it's *frame*.** Half the navigation leads to
worse pages than the home screen, and the design system has two sources of truth.

---

## 3. Design vision

> **"Mission control for a strategy you can trust."**

ViperTrade is not a retail trading toy; it is an **operator's instrument** for a
system that trades on its own. The operator's three jobs are: **(1) trust the
machine, (2) catch it when it drifts, (3) intervene safely.** Every pixel should
serve one of those. The console is therefore **calm by default, loud on signal**.

### Design principles

1. **Evidence over decoration.** Every number is traceable to an audit log, a
   consensus source, or a config value. Tooltips answer "why?" not "what?".
2. **Calm baseline, sharp alerts.** Glow and motion are *reserved* for state
   changes (a fill, a guard block, a thesis invalidation) — never ambient noise.
3. **Density with hierarchy.** Operators scan, then drill. Lead with the 5 numbers
   that matter; everything else is one interaction away.
4. **Real-time is the default, not a feature.** Live is the resting state; "stale"
   is the exception that must announce itself.
5. **Safe hands on the controls.** Destructive/market-moving actions (kill-switch,
   mode change, flag toggle) are deliberate, confirmed, and audit-logged.
6. **One design system, zero drift.** A single token source, a single component
   kit, a single header.

---

## 4. Brand & visual language

Build on the existing **Viper** identity (deep navy + electric cyan, profit-green,
strategy-purple) — but consolidate and add restraint.

### 4.1 Color — semantic, not decorative

```text
SURFACE          ACCENT / STATE
bg/abyss   #070e1c   cyan/strike   #00d4ff   ← primary action, live
bg/deep    #0a1120   green/profit  #00ff88   ← long, win, healthy
panel      #0f1b33   red/loss      #ef4444   ← short, loss, breach
panel/raised #152030 purple/brain  #a855f7   ← strategy / AI reasoning
line       rgba(95,137,203,.22) amber/watch  #f59e0b ← guard, attention
```

Rule: **hue = meaning.** Green is *always* profit/long/healthy; purple is *always*
strategy/AI; amber is *always* "a guard or threshold is in play". No exceptions.

### 4.2 Typography — three roles

- **Display / UI:** `Space Grotesk` (already loading) — confident, technical.
- **Numerics (tabular):** `JetBrains Mono` with `font-variant-numeric: tabular-nums`
  so columns of prices/PnL align to the digit. **This is non-negotiable for a
  trading UI** and is currently missing.
- **Body / dense tables:** `Inter`.

> Fix the drift: pick this set, declare it once, load via `next/font`, and delete
> the conflicting declarations.

### 4.3 Motion — earn every animation

| Trigger | Motion | Why |
|---|---|---|
| New fill / position open | 600ms cyan glow sweep on the row | Operator must *notice* |
| Guard blocks an entry | Amber pulse on the gauge, then settle | Explains a non-action |
| Live tick | 1px value flash (green up / red down) | Bloomberg-style, subliminal |
| Stale data (WS down) | Desaturate + "LIVE → STALE" badge | Failure must be loud |
| Idle dashboard | **Nothing moves** | Calm baseline |

### 4.4 Density modes

A global toggle: **Comfortable** (default, demo/review) vs **Cockpit** (compact
rows, smaller gutters, more on screen) — persisted per operator. Trading desks
live in Cockpit.

---

## 5. Information architecture

Collapse the dead-end sprawl into **5 real destinations**, each earning its place.

```text
┌─ ViperTrade ─────────────────────────────────────────────  ●LIVE  ⌘K  [PAPER ▾]  ⏻ ┐
│  Console   ·   Strategy   ·   Trades   ·   Analysis   ·   System                     │
└──────────────────────────────────────────────────────────────────────────────────┘
```

| Nav | Route | Purpose | Built from |
|---|---|---|---|
| **Console** | `/console` | The at-a-glance command center: portfolio, live positions, recent fills, system pulse | today's `/dashboard` top half |
| **Strategy** | `/strategy` | The Cockpit, full-screen: per-symbol consensus, RSI/%B/ADX gauges, guard decisions, *why this symbol is/ isn't trading* | today's Strategy Cockpit, expanded |
| **Trades** | `/trades` | Full trade ledger: filter/sort/group by reason, PnL attribution by close-reason (trailing vs thesis vs stop) | replaces the stub; uses existing trades API |
| **Analysis** | `/analysis` | Performance & evidence: equity curve, win-rate, allocation, backtest-vs-live deltas | today's `/analysis`, moved under the shared shell |
| **System** | `/system` | Architecture flow, service health, the **runtime flag panel** (`STRATEGY_REAL_DECISIONS`), kill-switch, mode (paper/testnet/mainnet) | today's Architecture Flow + new controls |

Removed from nav: standalone **Settings** stub (folds into **System** + an operator
menu). Removed from code: `layout/Header.tsx`, `layout/Footer.tsx` (orphans).

**Global affordances** (top bar, every page):

- **`●LIVE` health pill** — WS connection + data freshness, one glance.
- **`⌘K` command palette** — jump to any symbol, page, or action; toggle the flag;
  trip the kill-switch. The operator's superpower.
- **Mode selector `[PAPER ▾]`** — explicit, color-coded; switching to MAINNET
  demands confirmation.
- **`⏻` kill-switch** — always reachable, two-step confirm.

---

## 6. Page blueprints

### 6.1 Console (`/console`)

```text
┌────────────────────────────────────────────────────────────────────────────┐
│  EQUITY  $99.91   ▾ -0.29 (24h)        WIN RATE 54%   OPEN 0   TODAY 12 trades │  ← KPI strip (mono, tabular)
│  ┌──────────────── equity sparkline (24h) ──────────────────────────────┐   │
│  └──────────────────────────────────────────────────────────────────────┘   │
├──────────────────────────────┬─────────────────────────────────────────────┤
│  OPEN POSITIONS (live)        │  STRATEGY PULSE                              │
│  ┌─ none ─ "flat — guards     │  7 symbols · 3 ENTER-ready · 4 held by guard │
│  │  holding 16 setups today"  │  ▓▓▓▓░░░  consensus heatmap (per symbol)     │
│  └───────────────────────────┘│  → click any → Strategy view                 │
├──────────────────────────────┴─────────────────────────────────────────────┤
│  RECENT FILLS (live stream, newest glows in)                                 │
│  SUIUSDT  SHORT  +0.18  trailing_stop   2m ago                               │
│  XLMUSDT  LONG   -0.07  thesis_invalid  5m ago                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

The empty Open-Positions state becomes *informative*: "flat — guards holding N
setups" turns a blank into evidence the strategy is working as designed.

### 6.2 Strategy (`/strategy`) — the signature screen

Each symbol is a **decision card** that answers *"why is this trading or not?"*:

```text
┌ SUIUSDT ───────────────────────────────── consensus: bybit·binance·okx ──┐
│  ACTION  ⟶  HOLD            trend ADX 18 (weak)   min_adx 20 ✗            │
│  RSI  ────●────────  41     %B ──────────●──  0.62                        │
│        oversold  overbought        short-guard 0.15 ┆ long-guard 0.85     │
│  ▸ Why held: ADX 18 < min_adx 20  ·  %B 0.62 inside neutral band         │
└──────────────────────────────────────────────────────────────────────────┘
```

The guard zones on the gauges + the plain-language "Why held" line make the
strategy **self-explaining** — this is the auditability promise made visible, and
the natural home for the new ADX signal field.

### 6.3 Trades (`/trades`) — PnL attribution

The ledger, but with the insight the backtest taught us baked in: **group by
close-reason** and show net per reason (trailing = edge; thesis/stop = bleed).
Filters: symbol, side, mode, reason, date. Mono tabular columns, virtualized rows.

### 6.4 Analysis (`/analysis`)

Keep the content, move under the shared shell. Add the one chart operators ask for
first: **equity curve with trade markers** (hover a marker → the fill + its reason).
Add a **backtest-vs-live** strip: for each tuned lever, the backtest delta next to
the live delta — closing the "does config tuning match reality?" loop.

### 6.5 System (`/system`) — controls with consequences

```text
┌ RUNTIME ───────────────────────────────────────────────────────────────┐
│  STRATEGY_REAL_DECISIONS   ● ON   (runtime patch, not in git)   [toggle] │
│  MODE   PAPER   testnet   mainnet                                        │
│  KILL-SWITCH   ◯ armed                                  [TRIP — 2 step]  │
├ SERVICE HEALTH ─────────────────────────────────────────────────────────┤
│  market-data ●  strategy ●  executor ●  monitor ●  api ●  …  latencies   │
└──────────────────────────────────────────────────────────────────────────┘
```

Surfaces the operationally-critical, easily-forgotten runtime flag (today only
visible via `kubectl`) right in the UI, with an audit trail on every toggle.

---

## 7. Signature components

1. **`<Gauge>` (evolve `GaugeBar`)** — value + named guard zones + threshold ticks,
   one component for RSI / %B / ADX. The visual core of the auditability story.
2. **`<DecisionCard>`** — symbol → action + the reason it acted/held. New.
3. **`<LiveFeed>`** — WS-driven stream with "new item glows in" affordance.
4. **`<HealthPill>` / `<FreshnessBadge>`** — LIVE / STALE everywhere, one rule.
5. **`<MetricStat>`** — KPI with mono tabular value, trend arrow, sparkline,
   "why?" tooltip. Replaces ad-hoc KPI cards.
6. **`<CommandPalette>` (`⌘K`)** — navigate + act. New.
7. **`<DataTable>`** — sortable, filterable, virtualized, tabular-num — powers
   Trades and any future ledger.
8. **`<ConfirmAction>`** — the two-step guard for kill-switch / mode / flag.

All built on the existing shadcn/ui + CVA base; documented in a lightweight
`/system/components` gallery (or Storybook) so the system stops drifting.

---

## 8. Real-time & data architecture

- **WebSocket-first, REST-fallback** (the #51 pattern) becomes the *standard* for
  all live surfaces, not just the Cockpit. One `useLiveChannel(topic)` hook over
  the existing `viper:market_data` / `viper:decisions` Redis bridge.
- **TanStack Query** owns server state + cache + REST fallback; **Zustand** owns
  only UI/ephemeral state (density mode, palette open, selected symbol). Clear line.
- **Freshness contract:** every live value carries a timestamp; a shared
  `useFreshness()` flips the global pill to STALE past a threshold and desaturates
  affected panels. Failure is never silent.
- **Optimistic + audited controls:** flag/mode/kill-switch actions confirm against
  the API, then reflect immediately, and log who/when.

---

## 9. Operator ergonomics

- **Keyboard-native:** `⌘K` palette, `g s` → Strategy, `g t` → Trades, `/` to
  filter, number keys to jump symbols.
- **Deep-linkable state:** filters, selected symbol, density encoded in the URL —
  shareable, reload-safe.
- **Per-operator persistence:** density, default mode view, favorite symbols.
- **Responsive, not mobile-first:** the target is a desk monitor; degrade
  gracefully to a phone for on-call glance (health pill + kill-switch must work).

---

## 10. Accessibility & performance

- WCAG AA contrast on the navy theme (current cyan-on-navy passes; verify amber).
- Color is never the *only* signal — pair with icon/label (long/short, win/loss).
- Respect `prefers-reduced-motion` (kills the ambient flashes).
- Route-level code splitting; virtualized tables; `next/font` (no FOUT);
  target LCP < 1.5s on the console, interaction < 100ms.

---

## 11. Tech decisions

| Area | Keep | Add / change |
|---|---|---|
| Framework | Next 16 App Router | Shared `(console)` route group → one shell for all pages |
| UI kit | shadcn/ui + Tailwind + CVA | One token file; delete legacy CSS vars + orphan headers |
| Charts | Recharts | Keep; consider `visx`/`lightweight-charts` only if equity/candles need it |
| State | TanStack Query + Zustand | Formalize the server/UI split above |
| Fonts | — | `next/font`: Space Grotesk + JetBrains Mono + Inter |
| Realtime | `ws` + Redis bridge | Generalize into `useLiveChannel` |
| Quality | eslint/prettier/vitest | Add Playwright visual smoke + a component gallery |

---

## 12. Phased roadmap

**Phase 0 — Foundation & cleanup (≈2–3 days).** Single token source; `next/font` +
tabular numerics; delete `layout/Header.tsx`/`Footer.tsx`; introduce the shared
`(console)` route-group shell; move `/analysis` into it. *Outcome: zero drift, zero
dead ends — the stubs either get real or leave the nav.*

**Phase 1 — Console + Strategy (≈1 week).** Reframe `/dashboard` → `/console` with
the KPI strip + equity sparkline + LiveFeed + informative empty states. Full-screen
`/strategy` with `<DecisionCard>` and the ADX gauge. *Outcome: the two screens an
operator lives in.*

**Phase 2 — Trades + Analysis (≈1 week).** Real `<DataTable>` ledger with
close-reason PnL attribution; equity curve + backtest-vs-live in Analysis.
*Outcome: the evidence loop is complete.*

**Phase 3 — System & controls (≈3–4 days).** `/system` with runtime flag toggle,
mode selector, kill-switch (all audited), service health. Command palette `⌘K`.
*Outcome: safe in-UI operation, no more `kubectl` for the flag.*

**Phase 4 — Polish (ongoing).** Motion pass, density mode, keyboard map,
accessibility audit, component gallery, Playwright visual smoke in CI.

---

## 13. Quick wins (ship this week, independent of the big plan)

1. **Delete** `components/layout/Header.tsx` + `Footer.tsx` (dead, misleading nav).
2. **Demote the 3 stubs** from nav until real (or redirect to dashboard sections).
3. **Tabular numerics** on every price/PnL — 1 CSS line, big readability gain.
4. **Fix the font drift** — one declared family set via `next/font`.
5. **Informative empty state** — "flat — guards holding N setups" instead of
   "No open positions".
6. **`●LIVE/STALE` pill** in the header — instant trust signal.

---

*Built on the existing Viper brand and the live foundation already shipped
(Strategy Cockpit, WebSocket push, Architecture Flow). This proposal completes the
frame — it does not restart it.*
