'use client';

import { netPnl } from '@/lib/pnl';
import { useEffect, useMemo, useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { useDecisions } from '@/hooks/useDecisions';
import { useT, useLocale, formatNumber, formatUsd } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import { HudFrame } from '@/components/ui/HudFrame';
import { PageHeader } from '@/components/ui/PageHeader';
import { StatRail } from '@/components/ui/StatRail';
import { Sparkline } from '@/components/console/Sparkline';
import { MarketSentiment } from '@/components/console/MarketSentiment';
import { PositionGauge } from '@/components/console/PositionGauge';
import { LiveFeed } from '@/components/console/LiveFeed';
import { DeckSkeleton } from '@/components/console/DeckSkeleton';
import { EquityCurve } from '@/components/analysis/EquityCurve';
import { SwingMatrix, type SwingMatrixData } from '@/components/cockpit/SwingMatrix';

interface PositionItem {
  strategy_kind?: string;
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
  const locale = useLocale();

  const { data: dashboardData, loading } = useDashboard<DashboardData>(
    '/api/dashboard',
    { refreshInterval: 5000, enabled: true }
  );
  const { decisions, live } = useDecisions();
  // A matriz vem pronta do strategy: o checklist é avaliado a cada 60s no mesmo
  // ciclo que decide, então recalcular aqui só criaria uma segunda verdade.
  // 30s, não 15: o strategy só recalcula o checklist a cada 60s, então um
  // poll mais rápido gasta orçamento de requisição para reler o mesmo snapshot.
  const { data: swingMatrix } = useDashboard<SwingMatrixData>(
    '/api/strategy/swing-matrix',
    { refreshInterval: 30000, enabled: true }
  );

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
      running += netPnl(tr);
      points.push(running);
    }
    return points;
  }, [closedTrades, now]);

  // Esqueleto no formato do painel, em vez de um "Carregando…" centralizado que
  // esvaziava a tela inteira e a fazia saltar quando os dados chegavam.
  if (loading && !dashboardData) {
    return <DeckSkeleton />;
  }

  const openPositions = dashboardData?.positions?.items ?? [];
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


  return (
    <div className="space-y-4">
      <PageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        right={
          <div className="flex items-center gap-2 font-display text-2xs uppercase tracking-[0.25em] text-muted-foreground">
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
        }
      />

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
                  up
                    ? 'text-accent hud-glow-accent'
                    : 'text-destructive hud-glow-danger'
                )}
              >
                {up ? '▴' : '▾'} {formatUsd(locale, pnl24h)}{' '}
                <span className="text-2xs font-normal text-muted-foreground">
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
        marketSignals={marketSignals}
      />

      {/* Decision matrix — o checklist de swing que realmente decide */}
      <SwingMatrix data={swingMatrix} />
    </div>
  );
}
