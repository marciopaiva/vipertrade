'use client';

import { useDashboard } from '@/hooks/useDashboard';
import { useDecisions } from '@/hooks/useDecisions';
import { useT } from '@/lib/i18n';
import { PositionGauge } from '@/components/console/PositionGauge';
import { KpiStrip } from '@/components/console/KpiStrip';
import { MarketSentiment } from '@/components/console/MarketSentiment';
import { EquityCurve } from '@/components/analysis/EquityCurve';

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
  symbol: string;
  side: string;
  status: string;
  quantity: number;
  entry_price: number;
  exit_price?: number;
  pnl?: number;
  pnl_pct?: number;
  close_reason?: string;
  opened_at: string;
  closed_at?: string;
  duration_seconds?: number;
}

interface DashboardData {
  performance?: {
    last_24h?: { total_trades: number; total_pnl: number; win_rate: number };
  };
  positions?: { items: PositionItem[] };
  trades?: { items: TradeItem[] };
  daily_trades_summary?: { count?: number };
  wallet?: { total_equity?: number };
  // Loosely typed: PositionTable narrows these to its own MarketSignal shape.
  market_signals?: { items?: unknown[] | Record<string, unknown> };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type LooseSignal = any;

export default function ConsolePage() {
  const t = useT('console');
  const tc = useT('common');
  const { data: dashboardData, loading } = useDashboard<DashboardData>(
    '/api/dashboard',
    { refreshInterval: 5000, enabled: true }
  );
  // Live decisions power the PositionTable's "guards holding N setups" empty
  // state — the same %B gate the /strategy screen surfaces.
  const { decisions } = useDecisions();

  if (loading && !dashboardData) {
    return (
      <div className="flex min-h-[60vh] items-center justify-center">
        <div className="text-center">
          <div className="mb-2 text-2xl font-bold text-primary">{tc('loading')}</div>
          <div className="text-muted-foreground">{t('connecting')}</div>
        </div>
      </div>
    );
  }

  const openPositions = dashboardData?.positions?.items ?? [];
  const closedTrades = dashboardData?.trades?.items ?? [];
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

  return (
    <main className="container mx-auto space-y-4 px-4 py-4">
      {/* At-a-glance KPI strip — the single top-line source of truth. */}
      <KpiStrip
        equity={dashboardData?.wallet?.total_equity}
        pnl24h={dashboardData?.performance?.last_24h?.total_pnl}
        winRate24h={dashboardData?.performance?.last_24h?.win_rate}
        openCount={openPositions.length}
        todayCount={todayCount}
        trades={closedTrades}
      />

      {/* Equity curve — cumulative realized PnL, right under the KPI strip. */}
      <EquityCurve />

      {/* Market sentiment — full-width band: gauge on the left, Fear & Greed
          explanation + Long/Short ratio on the right. */}
      <MarketSentiment />

      {/* Open positions — horizontal risk-rail rows, one per token (full width). */}
      <PositionGauge
        positions={openPositions}
        guardedSetups={guardedSetups}
        marketSignals={marketSignals}
      />
    </main>
  );
}
