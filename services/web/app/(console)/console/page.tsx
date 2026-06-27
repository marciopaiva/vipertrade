'use client';

import { useEffect, useMemo, useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { useDecisions } from '@/hooks/useDecisions';
import { useT, useLocale, formatNumber, formatUsd } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import { HudFrame } from '@/components/ui/HudFrame';
import { StatRail } from '@/components/ui/StatRail';
import { Sparkline } from '@/components/console/Sparkline';
import { MarketSentiment } from '@/components/console/MarketSentiment';
import { PositionGauge } from '@/components/console/PositionGauge';
import { LiveFeed } from '@/components/console/LiveFeed';
import { EquityCurve } from '@/components/analysis/EquityCurve';
import { DecisionRow, ROW_GRID } from '@/components/cockpit/DecisionRow';

interface PositionItem {
  trade_id: string;
  symbol: string;
  side: string;
  quantity: number;
  notional_usdt: number;
  entry_price: number;
  opened_at?: string;
  stop_loss_price?: number;
  trailing_activation_price?: number;
  fixed_take_profit_price?: number;
  break_even_price?: number;
  trailing_stop_activated?: boolean;
  trailing_stop_peak_price?: number;
  trailing_stop_final_distance_pct?: number;
}

interface TradeItem {
  trade_id: string;
  status: string;
  pnl?: number;
  opened_at: string;
  closed_at?: string;
}

interface DashboardData {
  performance?: {
    last_24h?: { total_trades: number; total_pnl: number; win_rate: number };
  };
  positions?: { items: PositionItem[] };
  trades?: { items: TradeItem[] };
  daily_trades_summary?: { count?: number };
  wallet?: { total_equity?: number };
  market_signals?: { items?: unknown[] | Record<string, unknown> };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type LooseSignal = any;

export default function CommandDeckPage() {
  const t = useT('deck');
  const tc = useT('console');
  const tcm = useT('common');
  const ts = useT('strategy');
  const locale = useLocale();

  const { data: dashboardData, loading } = useDashboard<DashboardData>(
    '/api/dashboard',
    { refreshInterval: 5000, enabled: true }
  );
  const { decisions, live } = useDecisions();

  // Slide a 24h window forward so the equity sparkline stays honest on a
  // long-open page (Date.now() is impure during render).
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 60_000);
    return () => clearInterval(id);
  }, []);

  const closedTrades = useMemo(
    () => dashboardData?.trades?.items ?? [],
    [dashboardData]
  );

  // Cumulative realized PnL over the last 24h of closed trades — the equity
  // sparkline (no equity time-series exists in the API).
  const series = useMemo(() => {
    const since = now - 24 * 60 * 60 * 1000;
    const closed = closedTrades
      .filter(tr => {
        if (tr.status !== 'closed') return false;
        const ts2 = Date.parse(tr.closed_at || tr.opened_at);
        return Number.isFinite(ts2) && ts2 >= since;
      })
      .sort(
        (a, b) =>
          Date.parse(a.closed_at || a.opened_at) -
          Date.parse(b.closed_at || b.opened_at)
      );
    let running = 0;
    const points = [0];
    for (const tr of closed) {
      running += tr.pnl ?? 0;
      points.push(running);
    }
    return points;
  }, [closedTrades, now]);

  if (loading && !dashboardData) {
    return (
      <div className="flex min-h-[60vh] items-center justify-center">
        <div className="text-center">
          <div className="mb-2 text-2xl font-bold text-primary hud-glow">
            {tcm('loading')}
          </div>
          <div className="text-muted-foreground">{tc('connecting')}</div>
        </div>
      </div>
    );
  }

  const openPositions = dashboardData?.positions?.items ?? [];
  const guardedSetups = decisions.filter(d => {
    const pb = d.consensus_bollinger_percent_b;
    return typeof pb === 'number' && (pb > 0.85 || pb < 0.15);
  }).length;
  const todayCount =
    dashboardData?.daily_trades_summary?.count ??
    dashboardData?.performance?.last_24h?.total_trades ??
    0;
  const marketSignals = dashboardData?.market_signals?.items
    ? (Object.values(dashboardData.market_signals.items) as LooseSignal[])
    : [];

  const equity = dashboardData?.wallet?.total_equity;
  const pnl24h = dashboardData?.performance?.last_24h?.total_pnl ?? 0;
  const winRate = dashboardData?.performance?.last_24h?.win_rate;
  const up = pnl24h >= 0;

  // Entering symbols float to the top (the actionable ones), then alphabetical.
  const ordered = [...decisions].sort((a, b) => {
    const ae = a.action.startsWith('ENTER') ? 0 : 1;
    const be = b.action.startsWith('ENTER') ? 0 : 1;
    return ae - be || a.symbol.localeCompare(b.symbol);
  });

  return (
    <div className="space-y-4">
      {/* Status bar */}
      <div className="flex items-center gap-2 font-display text-[11px] uppercase tracking-[0.25em] text-muted-foreground">
        <span className="relative flex h-2 w-2">
          {live && (
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent opacity-60" />
          )}
          <span
            className={cn(
              'relative inline-flex h-2 w-2 rounded-full',
              live ? 'bg-accent' : 'bg-muted-foreground'
            )}
          />
        </span>
        {t('statusLive', { n: decisions.length })}
      </div>

      {/* Instrument cluster — equity + sentiment dial. Win rate / net / open /
          today live in the rail below, so nothing is shown twice. */}
      <div className="grid gap-4 lg:grid-cols-2">
        <HudFrame title={t('equity')} scan>
          <div className="flex items-end gap-4">
            <div className="shrink-0">
              <div className="font-mono text-4xl font-bold tabular-nums tracking-tight text-foreground">
                {typeof equity === 'number'
                  ? `$${formatNumber(locale, equity)}`
                  : '—'}
              </div>
              <div
                className={cn(
                  'mt-1 font-mono text-sm font-semibold tabular-nums',
                  up ? 'text-accent hud-glow-accent' : 'text-destructive hud-glow-danger'
                )}
              >
                {up ? '▴' : '▾'} {formatUsd(locale, pnl24h)}{' '}
                <span className="text-[11px] font-normal text-muted-foreground">
                  24h
                </span>
              </div>
            </div>
            <Sparkline
              values={series}
              colorClassName={up ? 'text-accent' : 'text-destructive'}
              className="h-16 flex-1"
            />
          </div>
        </HudFrame>

        <HudFrame title={t('sentiment')}>
          <MarketSentiment />
        </HudFrame>
      </div>

      {/* KPI rail — the single source for win / open / today (net is on the
          equity instrument above). */}
      <HudFrame>
        <StatRail
          items={[
            {
              label: tc('winRate'),
              value:
                typeof winRate === 'number'
                  ? `${formatNumber(locale, winRate, 0)}%`
                  : '—',
              tone: (winRate ?? 0) >= 50 ? 'accent' : 'warn',
            },
            { label: tc('open'), value: openPositions.length },
            { label: tc('today'), value: todayCount },
          ]}
        />
      </HudFrame>

      {/* Equity curve + live feed */}
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <EquityCurve />
        </div>
        <HudFrame title={t('liveFeed')} scan>
          <LiveFeed />
        </HudFrame>
      </div>

      {/* Open positions — risk rail (self-framed) */}
      <PositionGauge
        positions={openPositions}
        guardedSetups={guardedSetups}
        marketSignals={marketSignals}
      />

      {/* Decision matrix (folded from /strategy) */}
      <HudFrame title={t('decisionMatrix')}>
        {decisions.length === 0 ? (
          <div className="px-3 py-10 text-center text-sm text-muted-foreground">
            {ts('empty')}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <div
              className={cn(
                ROW_GRID,
                'border-b border-border px-3 py-2 text-[10px] uppercase tracking-[0.15em] text-muted-foreground'
              )}
            >
              <span>{ts('colSymbol')}</span>
              <span>{ts('colState')}</span>
              <span>{ts('colConsensus')}</span>
              <span>{ts('colRsi')}</span>
              <span>{ts('colPb')}</span>
              <span>{ts('colAdx')}</span>
              <span>{ts('colWhy')}</span>
            </div>
            {ordered.map(d => (
              <DecisionRow key={d.symbol} d={d} />
            ))}
          </div>
        )}
      </HudFrame>
    </div>
  );
}
