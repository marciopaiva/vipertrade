'use client';

import { useCallback, useMemo, useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { useDecisions } from '@/hooks/useDecisions';
import ServiceFlowDiagram from '@/components/dashboard/ServiceFlowDiagram';
import { PositionTable } from '@/components/dashboard/PositionTable';
import { StrategyCockpit } from '@/components/cockpit/StrategyCockpit';
import { KpiStrip } from '@/components/console/KpiStrip';
import { RecentFills } from '@/components/console/RecentFills';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

// Types
interface MarketSignal {
  symbol: string;
  current_price: number;
  atr_14: number;
  volume_24h: number;
  funding_rate: number;
  trend_score: number;
  spread_pct: number;
  regime?: string;
  consensus_side?: string;
  consensus_count?: number;
  exchanges_available?: number;
  bybit_regime?: string;
}

interface TokenDecision {
  symbol: string;
  regime: string;
  consensusSide: string;
  consensusLabel: string;
  bybitRegime: string;
  consensusCount: number;
  exchangesAvailable: number;
  trendScore: number;
  stateLabel: string;
  stateTone: 'positive' | 'negative' | 'neutral';
  stateContext?: string;
  bybitAligned: boolean;
  hasDivergence: boolean;
}

interface FlowContext {
  strategySymbol?: string;
  strategyState?: string;
  strategyContext?: string;
  executorSymbol?: string;
  executorAction?: string;
  executorContext?: string;
}

interface AnalystEvaluationSignal {
  reason?: string;
  severity?: 'pass' | 'warn' | 'fail';
  dominant_gate?: string;
  symbol?: string;
  recommendation?: string;
  top_reason?: string;
  thesis_invalidated_pct?: number;
  trailing_stop_pct?: number;
}

interface AiAnalystData {
  generated_at?: string;
  lookback_hours?: number;
  heuristic_summary?: string;
  llm_summary?: string | null;
  tupa_error?: string | null;
  summary?: {
    closed_trades?: number;
    total_pnl_usdt?: number;
    avg_pnl_pct?: number;
    avg_duration_s?: number;
    win_rate_pct?: number;
  };
  expectancy?: {
    winning_trades?: number;
    losing_trades?: number;
    neutral_trades?: number;
    avg_win_usdt?: number;
    avg_win_pct?: number;
    avg_loss_usdt?: number;
    avg_loss_pct?: number;
    payoff_ratio?: number;
    expectancy_usdt?: number;
    expectancy_pct?: number;
  };
  comparative_diagnostics?: {
    status?: string;
    reasons?: string[];
    current_window_hours?: number;
    previous_window_hours?: number;
    closed_trades?: { current?: number; previous?: number; delta?: number };
    win_rate_pct?: { current?: number; previous?: number; delta?: number };
    expectancy_pct?: { current?: number; previous?: number; delta?: number };
    payoff_ratio?: { current?: number; previous?: number; delta?: number };
    thesis_invalidated_pct?: {
      current?: number;
      previous?: number;
      delta?: number;
    };
    trailing_stop_pct?: { current?: number; previous?: number; delta?: number };
    long_avg_pnl_pct?: { current?: number; previous?: number; delta?: number };
    short_avg_pnl_pct?: { current?: number; previous?: number; delta?: number };
  };
  recommendations?: Array<{
    recommendation_id?: string;
    severity?: 'pass' | 'warn' | 'fail' | 'info' | string;
    confidence?: string;
    recommendation?: string;
    evidence?: string;
    expected_tradeoff?: string;
  }>;
  hypotheses?: Array<{
    hypothesis_id?: string;
    priority?: string;
    confidence?: string;
    hypothesis?: string;
    evidence?: string;
    observe?: string;
    success_condition?: string;
    failure_condition?: string;
  }>;
  symbol_diagnostics?: Array<{
    symbol?: string;
    status?: string;
    recommendation?: string;
    confidence?: string;
    trades?: number;
    avg_pnl_pct?: number;
    thesis_invalidated_trades?: number;
    trailing_stop_trades?: number;
    avg_thesis_pnl_pct?: number;
    avg_trailing_pnl_pct?: number;
  }>;
  regime_diagnostics?: {
    status?: string;
    regime?: string;
    confidence?: string;
    directional_bias?: string;
    dominant_gate?: string;
    exit_profile?: string;
    evidence?: string[];
  };
  tupa_evaluation?: {
    exit_pressure?: AnalystEvaluationSignal;
    directional_bias?: AnalystEvaluationSignal;
    entry_pressure?: AnalystEvaluationSignal;
    thesis_quality?: AnalystEvaluationSignal;
    symbol_risk?: AnalystEvaluationSignal;
    summary?: {
      closed_trades?: number;
      total_pnl_usdt?: number;
      avg_pnl_pct?: number;
      avg_duration_s?: number;
      win_rate_pct?: number;
    };
  };
}

interface DashboardData {
  status: {
    trading_mode: string;
    trading_profile: string;
    trade_profile_label?: string;
    risk_status: string;
    db_connected: boolean;
    executor: { enabled: boolean; reason?: string };
    kill_switch: { enabled: boolean };
    risk_limits: {
      max_daily_loss_pct: number;
      max_leverage: number;
      risk_per_trade_pct: number;
    };
  };
  performance: {
    last_24h?: { total_trades: number; total_pnl: number; win_rate: number };
    last_7d?: { total_trades: number; total_pnl: number; win_rate: number };
  };
  positions: {
    items: Array<{
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
    }>;
  };
  trades: {
    items: Array<{
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
    }>;
  };
  daily_trades_summary?: { count?: number };
  wallet?: {
    total_equity?: number;
    wallet_balance?: number;
    margin_balance?: number;
    available_balance?: number;
    unrealized_pnl?: number;
    initial_margin?: number;
    maintenance_margin?: number;
    account_im_rate?: number;
    account_mm_rate?: number;
    account_type?: string;
  };
  services: Array<{ name: string; ok: boolean; latency_ms: number }>;
  events?: {
    items?: Array<{
      event_id: string;
      event_type: string;
      severity: string;
      timestamp: string;
      symbol?: string;
      data?: any;
    }>;
  };
  market_signals?: { items?: any[] | Record<string, any> };
  ai_analyst?: AiAnalystData;
  partial?: boolean;
  warnings?: string[];
}

// Helper functions
function num(value: number | null | undefined, digits = 2) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return value.toFixed(digits);
}

function usd(value: number | null | undefined) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 2,
  }).format(value);
}

function pct(value: number | null | undefined) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return `${(value * 100).toFixed(2)}%`;
}

function titleCase(value: string | null | undefined) {
  if (!value) return '';
  return value
    .replaceAll('_', ' ')
    .toLowerCase()
    .replace(/\b\w/g, char => char.toUpperCase());
}

function toneClasses(severity?: string) {
  if (severity === 'fail') {
    return {
      badge: 'border-destructive/35 bg-destructive/10 text-destructive',
      text: 'text-destructive',
    };
  }
  if (severity === 'warn') {
    return {
      badge: 'border-primary/35 bg-primary/10 text-primary',
      text: 'text-primary',
    };
  }
  return {
    badge: 'border-accent/35 bg-accent/10 text-accent',
    text: 'text-accent',
  };
}

function comparativeTone(status?: string) {
  if (status === 'regressed') return toneClasses('fail');
  if (status === 'mixed' || status === 'insufficient_baseline')
    return toneClasses('warn');
  return toneClasses('pass');
}

// Components
function WalletCard({
  label,
  amount,
  rate,
  accent = '#11c4ff',
}: {
  label: string;
  amount?: number;
  rate?: number;
  accent?: string;
}) {
  return (
    <Card className="bg-card border-border">
      <CardContent className="pt-6">
        <div className="text-xs uppercase tracking-wider text-muted-foreground mb-2">
          {label}
        </div>
        <div className="text-2xl font-bold" style={{ color: accent }}>
          {usd(amount)}
        </div>
        {rate !== undefined && (
          <div className="text-xs text-muted-foreground mt-1">{pct(rate)}</div>
        )}
      </CardContent>
    </Card>
  );
}

function MetricCard({
  label,
  value,
  accent = '#11c4ff',
  helper,
}: {
  label: string;
  value: string | number;
  accent?: string;
  helper?: string;
}) {
  return (
    <Card className="bg-card border-border">
      <CardContent className="pt-6">
        <div className="text-xs uppercase tracking-wider text-muted-foreground mb-2">
          {label}
        </div>
        <div className="text-2xl font-bold" style={{ color: accent }}>
          {value}
        </div>
        {helper && (
          <div className="text-xs text-muted-foreground mt-1">{helper}</div>
        )}
      </CardContent>
    </Card>
  );
}

function ServicesGrid({
  services,
}: {
  services: Array<{ name: string; ok: boolean; latency_ms: number }>;
}) {
  const flowOrder = [
    'bybit',
    'market-data',
    'strategy',
    'executor',
    'api',
    'monitor',
    'analytics',
  ];
  const sortedServices = [...services].sort((a, b) => {
    const aIndex = flowOrder.findIndex(f => a.name.includes(f));
    const bIndex = flowOrder.findIndex(f => b.name.includes(f));
    return (aIndex === -1 ? 99 : aIndex) - (bIndex === -1 ? 99 : bIndex);
  });

  return (
    <Card className="border-0 bg-transparent shadow-none [&>*]:px-0">
      <CardHeader>
        <CardTitle className="text-lg">Services Flow</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-3">
          {sortedServices.map(service => (
            <div
              key={service.name}
              className={cn(
                'p-3 rounded-lg border text-center',
                service.ok
                  ? 'border-accent/30 bg-accent/10'
                  : 'border-destructive/30 bg-destructive/10'
              )}
            >
              <div className="text-xs text-muted-foreground capitalize truncate">
                {service.name}
              </div>
              <div
                className={cn(
                  'text-sm font-semibold mt-1',
                  service.ok ? 'text-accent' : 'text-destructive'
                )}
              >
                {service.ok ? '✓' : '✗'}
              </div>
              {service.latency_ms > 0 && (
                <div className="text-xs text-muted-foreground mt-1">
                  {service.latency_ms}ms
                </div>
              )}
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function ClosedTradesTable({
  trades,
}: {
  trades: Array<{
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
  }>;
}) {
  const [selectedDay, setSelectedDay] = useState<string>('');
  const [page, setPage] = useState(0);
  const [nowMs] = useState(() => Date.now());
  const pageSize = 10;

  // Filter closed trades from last 7 days
  const closedTrades = useMemo(() => {
    const sevenDaysAgo = nowMs - 7 * 24 * 60 * 60 * 1000;
    return trades
      .filter(t => {
        if (t.status !== 'closed') return false;
        const refTime = Date.parse(t.closed_at || t.opened_at);
        return Number.isFinite(refTime) && refTime >= sevenDaysAgo;
      })
      .sort((a, b) => {
        const timeA = Date.parse(b.closed_at || b.opened_at);
        const timeB = Date.parse(a.closed_at || a.opened_at);
        return timeA - timeB;
      });
  }, [nowMs, trades]);

  // Group by day
  const tradesByDay = useMemo(() => {
    const groups = new Map<
      string,
      {
        key: string;
        label: string;
        items: typeof closedTrades;
        latestTs: number;
      }
    >();

    closedTrades.forEach(trade => {
      const refTime = Date.parse(trade.closed_at || trade.opened_at);
      if (!Number.isFinite(refTime)) return;

      const closedAt = new Date(refTime);
      const key = closedAt.toISOString().slice(0, 10);
      const label = closedAt.toLocaleDateString();
      const current = groups.get(key);

      if (current) {
        current.items.push(trade);
        current.latestTs = Math.max(current.latestTs, refTime);
      } else {
        groups.set(key, { key, label, items: [trade], latestTs: refTime });
      }
    });

    return Array.from(groups.values()).sort((a, b) => b.latestTs - a.latestTs);
  }, [closedTrades]);

  // Active day
  const activeDay = useMemo(() => {
    if (selectedDay && tradesByDay.some(g => g.key === selectedDay)) {
      return selectedDay;
    }
    return tradesByDay[0]?.key || '';
  }, [tradesByDay, selectedDay]);

  const activeGroup = useMemo(
    () => tradesByDay.find(g => g.key === activeDay) || null,
    [activeDay, tradesByDay]
  );
  const totalPages = useMemo(
    () => Math.max(1, Math.ceil((activeGroup?.items.length || 0) / pageSize)),
    [activeGroup]
  );
  const paginatedTrades = useMemo(() => {
    if (!activeGroup) return [];
    const start = page * pageSize;
    return activeGroup.items.slice(start, start + pageSize);
  }, [activeGroup, page]);

  if (closedTrades.length === 0) {
    return (
      <Card className="border-0 bg-transparent shadow-none [&>*]:px-0">
        <CardHeader className="pb-2">
          <CardTitle className="text-lg text-foreground">
            Recent Closed Trades
          </CardTitle>
        </CardHeader>
        <CardContent className="pt-0">
          <div className="text-center text-muted-foreground py-8">
            No closed trades in the last 7 days
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="border-0 bg-transparent shadow-none [&>*]:px-0">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg text-foreground">
            Recent Closed Trades
          </CardTitle>
          <Badge
            variant="outline"
            className="text-xs border-border text-muted-foreground"
          >
            Last 7 days
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="pt-0">
        {/* Day selector - compact */}
        {tradesByDay.length > 1 && (
          <div className="flex gap-1 mb-3 flex-wrap">
            {tradesByDay.map(day => (
              <Button
                key={day.key}
                variant={activeDay === day.key ? 'default' : 'outline'}
                size="sm"
                onClick={() => {
                  setSelectedDay(day.key);
                  setPage(0);
                }}
                className="text-xs px-2 py-1 h-7"
              >
                {day.label.split(',')[0]} ({day.items.length})
              </Button>
            ))}
          </div>
        )}

        <div className="hidden xl:grid xl:grid-cols-[220px_70px_110px_110px_110px_120px_1fr] gap-4 px-3 pb-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
          <div>Asset</div>
          <div>Side</div>
          <div className="text-right">PnL</div>
          <div>Entry</div>
          <div>Exit</div>
          <div>Closed</div>
          <div>Reason</div>
        </div>

        {/* Trades list - compact */}
        <div className="space-y-2">
          {paginatedTrades.map(trade => {
            const pnl = trade.pnl || 0;
            const pnlColor = pnl >= 0 ? '#10b981' : '#ef4444';
            const sideColor = trade.side === 'Long' ? '#10b981' : '#ef4444';
            const durationLabel = trade.duration_seconds
              ? `${Math.max(1, Math.round(trade.duration_seconds / 60))}m`
              : '-';
            const reasonLabel = titleCase(trade.close_reason || 'unknown');
            const closedAt = trade.closed_at ? new Date(trade.closed_at) : null;
            const closedDateLabel = closedAt
              ? closedAt.toLocaleDateString(undefined, {
                  month: 'short',
                  day: 'numeric',
                })
              : '-';
            const closedTimeLabel = closedAt
              ? closedAt.toLocaleTimeString([], {
                  hour: '2-digit',
                  minute: '2-digit',
                })
              : '-';

            return (
              <div
                key={trade.trade_id}
                className="bg-secondary/50 rounded-lg border border-border p-3"
              >
                <div className="grid grid-cols-1 gap-4 xl:grid-cols-[220px_70px_110px_110px_110px_120px_1fr] xl:items-center">
                  <div className="min-w-0">
                    <div className="text-sm font-bold text-foreground">
                      {trade.symbol}
                    </div>
                  </div>

                  <div>
                    <Badge
                      style={{
                        backgroundColor: sideColor + '22',
                        color: sideColor,
                        borderColor: sideColor + '55',
                      }}
                      className="h-5 min-w-[58px] justify-center px-1.5 py-0.5 text-[10px]"
                    >
                      {trade.side.toUpperCase()}
                    </Badge>
                  </div>

                  <div className="text-right xl:pr-2">
                    <div
                      className="text-sm font-bold"
                      style={{ color: pnlColor }}
                    >
                      {pnl ? `$${pnl.toFixed(2)}` : '-'}
                    </div>
                    <div className="text-xs" style={{ color: pnlColor }}>
                      {trade.pnl_pct
                        ? `${(trade.pnl_pct * 100).toFixed(2)}%`
                        : '-'}
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-2 text-xs md:contents">
                    <div>
                      <Badge
                        className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                        style={{
                          backgroundColor: '#0f172acc',
                          color: '#e2e8f0',
                          borderColor: '#334155',
                        }}
                      >
                        ${trade.entry_price.toFixed(6)}
                      </Badge>
                    </div>
                    <div>
                      {trade.exit_price ? (
                        <Badge
                          className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                          style={{
                            backgroundColor: '#0f172acc',
                            color: '#e2e8f0',
                            borderColor: '#334155',
                          }}
                        >
                          ${trade.exit_price.toFixed(6)}
                        </Badge>
                      ) : (
                        <div className="flex h-6 items-center justify-center text-muted-foreground">
                          -
                        </div>
                      )}
                    </div>
                  </div>

                  <div>
                    <div className="text-xs text-foreground">
                      {closedDateLabel}
                    </div>
                    <div className="text-[11px] text-muted-foreground">
                      {closedTimeLabel}
                    </div>
                  </div>

                  <div className="flex items-center gap-3 text-xs">
                    <span className="text-foreground">{reasonLabel}</span>
                    {durationLabel !== '-' && (
                      <>
                        <span className="text-muted-foreground">·</span>
                        <span className="text-muted-foreground">{durationLabel}</span>
                      </>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>

        {/* Pagination - compact */}
        {activeGroup && activeGroup.items.length > pageSize && (
          <div className="flex items-center justify-between mt-3">
            <div className="text-xs text-muted-foreground">
              {activeGroup.label} · {activeGroup.items.length} trades · p.
              {page + 1}/{totalPages}
            </div>
            <div className="flex gap-1">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage(p => Math.max(0, p - 1))}
                disabled={page === 0}
                className="text-xs px-2 py-1 h-7"
              >
                Prev
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
                disabled={page >= totalPages - 1}
                className="text-xs px-2 py-1 h-7"
              >
                Next
              </Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default function ConsolePage() {
  const {
    data: dashboardData,
    loading,
    error,
  } = useDashboard<DashboardData>('/api/dashboard', {
    refreshInterval: 5000,
    enabled: true,
  });
  // Live decisions power the "guards holding N setups" empty state — same %B
  // gate the /strategy screen surfaces.
  const { decisions } = useDecisions();
  const [lastStableMarketSignals, setLastStableMarketSignals] = useState<
    Record<string, any>
  >({});

  const liveMarketSignals = useMemo<Record<string, any>>(() => {
    const items = dashboardData?.market_signals?.items;
    if (!items) return {};
    if (Array.isArray(items)) {
      return Object.fromEntries(
        items.map((signal: any) => [signal.symbol, signal])
      );
    }
    return items as Record<string, any>;
  }, [dashboardData?.market_signals?.items]);

  // Cache the last non-empty snapshot so the cockpit doesn't flicker when a
  // refresh momentarily returns no signals. Adjusting state during render
  // (guarded so it can't loop) is React's recommended alternative to a
  // setState-in-effect — liveMarketSignals is a stable memoized reference, so
  // once it equals the cache the guard stops firing.
  if (
    Object.keys(liveMarketSignals).length > 0 &&
    liveMarketSignals !== lastStableMarketSignals
  ) {
    setLastStableMarketSignals(liveMarketSignals);
  }

  const effectiveMarketSignals = useMemo<Record<string, any>>(() => {
    if (Object.keys(liveMarketSignals).length > 0) return liveMarketSignals;
    return lastStableMarketSignals;
  }, [lastStableMarketSignals, liveMarketSignals]);

  // Build token priority from market signals
  const tokenDecisions = useMemo<TokenDecision[]>(() => {
    const signalsObj = effectiveMarketSignals;
    const signalsArray = Object.values(signalsObj);
    if (signalsArray.length === 0) return [];

    const events = dashboardData?.events?.items || [];
    const positions = dashboardData?.positions?.items || [];

    const latestExecutorEventBySymbol = new Map<
      string,
      { action?: string; status?: string }
    >();
    for (const event of events) {
      if (
        event.event_type !== 'executor_event_processed' ||
        !event.symbol ||
        latestExecutorEventBySymbol.has(event.symbol)
      ) {
        continue;
      }
      latestExecutorEventBySymbol.set(event.symbol, {
        action: event.data?.action ? String(event.data.action) : undefined,
        status: event.data?.status ? String(event.data.status) : undefined,
      });
    }

    return signalsArray
      .map((signal: any) => {
        const consensusSide =
          signal.consensus_side || signal.regime || 'neutral';
        const consensusCount = signal.consensus_count || 0;
        const exchanges = signal.exchanges_available || 0;
        const executorEvent = latestExecutorEventBySymbol.get(signal.symbol);
        const openPosition = positions.find(
          position => position.symbol === signal.symbol
        );

        let stateLabel = 'Watching';
        let stateTone: 'positive' | 'negative' | 'neutral' = 'neutral';
        let stateContext: string | undefined;
        let consensusLabel = 'Mixed Consensus';

        if (consensusSide === 'bullish' && consensusCount >= 2) {
          consensusLabel = 'Bullish Consensus';
        } else if (consensusSide === 'bearish' && consensusCount >= 2) {
          consensusLabel = 'Bearish Consensus';
        } else if (consensusSide === 'bullish') {
          consensusLabel = 'Bullish Watch';
        } else if (consensusSide === 'bearish') {
          consensusLabel = 'Bearish Watch';
        }

        if (openPosition) {
          stateLabel = `Open ${String(openPosition.side)}`;
          stateTone =
            String(openPosition.side).toLowerCase() === 'long'
              ? 'positive'
              : 'negative';
          stateContext = `${usd(openPosition.notional_usdt)} live`;
        } else if (executorEvent?.status === 'paper_open') {
          const action = String(executorEvent.action || '').toUpperCase();
          stateLabel =
            action === 'ENTER_LONG'
              ? 'Enter Long'
              : action === 'ENTER_SHORT'
                ? 'Enter Short'
                : 'Opening';
          stateTone =
            action === 'ENTER_LONG'
              ? 'positive'
              : action === 'ENTER_SHORT'
                ? 'negative'
                : 'neutral';
          stateContext = 'Executor accepted';
        } else if (executorEvent?.status === 'ignored_hold') {
          stateLabel = 'Hold';
          stateTone = 'neutral';
          stateContext = 'Strategy blocked entry';
        } else if (executorEvent?.status?.startsWith('blocked_')) {
          stateLabel = 'Blocked';
          stateTone = 'neutral';
          stateContext = titleCase(executorEvent.status);
        }

        const bybitAligned =
          consensusSide !== 'neutral' &&
          (signal.bybit_regime || 'neutral') === consensusSide;
        const hasDivergence = exchanges > 0 && consensusCount < exchanges;

        return {
          symbol: signal.symbol,
          regime: signal.regime || 'neutral',
          consensusSide,
          consensusLabel,
          bybitRegime: signal.bybit_regime || 'neutral',
          consensusCount,
          exchangesAvailable: exchanges,
          trendScore: signal.trend_score || 0,
          stateLabel,
          stateTone,
          stateContext,
          bybitAligned,
          hasDivergence,
        };
      })
      .sort((a: any, b: any) => {
        const tonePriority: Record<string, number> = {
          positive: 0,
          negative: 1,
          neutral: 2,
        };
        return (
          tonePriority[a.stateTone] - tonePriority[b.stateTone] ||
          b.consensusCount - a.consensusCount ||
          Math.abs(b.trendScore) - Math.abs(a.trendScore)
        );
      });
  }, [
    dashboardData?.events?.items,
    dashboardData?.positions?.items,
    effectiveMarketSignals,
  ]);

  const flowContext = useMemo<FlowContext>(() => {
    const signalsObj = effectiveMarketSignals;
    const latestExecutorEvent = (dashboardData?.events?.items || []).find(
      event => event.event_type === 'executor_event_processed' && event.symbol
    );
    const leadToken = latestExecutorEvent?.symbol
      ? tokenDecisions.find(
          token => token.symbol === latestExecutorEvent.symbol
        ) || tokenDecisions[0]
      : tokenDecisions[0];
    const leadSignal = leadToken ? signalsObj[leadToken.symbol] : null;

    const openPosition = (dashboardData?.positions?.items || [])[0];
    const lastClosedTrade = (dashboardData?.trades?.items || []).find(
      trade => trade.status === 'closed'
    );

    const strategyState = leadToken ? leadToken.stateLabel : 'scan idle';
    const strategyContext = leadSignal
      ? leadToken?.stateContext ||
        `${leadSignal.consensus_count || 0}/${leadSignal.exchanges_available || 0} consensus`
      : undefined;

    const executorSymbol =
      openPosition?.symbol ||
      latestExecutorEvent?.symbol ||
      lastClosedTrade?.symbol;

    const executorAction = openPosition
      ? `open ${openPosition.side.toLowerCase()}`
      : latestExecutorEvent?.data?.action
        ? String(latestExecutorEvent.data.action).toLowerCase()
        : 'idle';

    const executorContext = openPosition
      ? `${usd(openPosition.notional_usdt)} live`
      : latestExecutorEvent?.data?.status
        ? String(latestExecutorEvent.data.status).replaceAll('_', ' ')
        : 'awaiting decision';

    return {
      strategySymbol: leadToken?.symbol,
      strategyState,
      strategyContext,
      executorSymbol,
      executorAction,
      executorContext,
    };
  }, [
    dashboardData?.events?.items,
    dashboardData?.positions?.items,
    dashboardData?.trades?.items,
    effectiveMarketSignals,
    tokenDecisions,
  ]);

  if (loading && !dashboardData) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <div className="text-2xl font-bold text-primary mb-2">Loading...</div>
          <div className="text-muted-foreground">Connecting to ViperTrade</div>
        </div>
      </div>
    );
  }

  const tradingMode =
    (dashboardData?.status?.trading_mode?.toLowerCase() as
      | 'paper'
      | 'testnet'
      | 'mainnet') || 'paper';
  const executorEnabled = dashboardData?.status?.executor?.enabled ?? false;

  const openPositions = dashboardData?.positions?.items || [];
  const closedTrades = dashboardData?.trades?.items || [];
  const guardedSetups = decisions.filter(d => {
    const pb = d.consensus_bollinger_percent_b;
    return typeof pb === 'number' && (pb > 0.85 || pb < 0.15);
  }).length;
  const todayCount =
    dashboardData?.daily_trades_summary?.count ??
    dashboardData?.performance?.last_24h?.total_trades ??
    0;

  return (
    <div className="min-h-screen bg-background">
      {/* Main Content */}
      <main className="container mx-auto px-4 py-4 space-y-4">
        {/* At-a-glance KPI strip */}
        <KpiStrip
          equity={dashboardData?.wallet?.total_equity}
          pnl24h={dashboardData?.performance?.last_24h?.total_pnl}
          winRate24h={dashboardData?.performance?.last_24h?.win_rate}
          openCount={openPositions.length}
          todayCount={todayCount}
          trades={closedTrades}
        />

        {/* Wallet Card - Unified */}
        <Card className="border-0 bg-transparent shadow-none [&>*]:px-0">
          <CardHeader className="pb-1">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base text-foreground">
                Wallet Overview
              </CardTitle>
            </div>
          </CardHeader>
          <CardContent className="pt-0 space-y-4">
            <div className="relative overflow-hidden rounded-[28px] border border-border bg-[radial-gradient(circle_at_top_right,rgba(16,185,129,0.16),transparent_28%),linear-gradient(180deg,rgba(15,23,42,0.74),rgba(15,23,42,0.42))] px-6 py-5">
              <div className="absolute right-4 top-4 hidden sm:block">
                <svg
                  width="120"
                  height="56"
                  viewBox="0 0 120 56"
                  className="opacity-80"
                >
                  <defs>
                    <linearGradient
                      id="walletLine"
                      x1="0%"
                      y1="0%"
                      x2="100%"
                      y2="0%"
                    >
                      <stop offset="0%" stopColor="#10b981" stopOpacity="0.2" />
                      <stop
                        offset="100%"
                        stopColor="#34d399"
                        stopOpacity="0.95"
                      />
                    </linearGradient>
                    <linearGradient
                      id="walletFill"
                      x1="0%"
                      y1="0%"
                      x2="0%"
                      y2="100%"
                    >
                      <stop
                        offset="0%"
                        stopColor="#10b981"
                        stopOpacity="0.28"
                      />
                      <stop offset="100%" stopColor="#10b981" stopOpacity="0" />
                    </linearGradient>
                  </defs>
                  <path
                    d="M10 42 L34 36 L58 30 L82 18 L110 6"
                    fill="none"
                    stroke="url(#walletLine)"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                  />
                  <path
                    d="M10 42 L34 36 L58 30 L82 18 L110 6 L110 56 L10 56 Z"
                    fill="url(#walletFill)"
                  />
                </svg>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <div className="text-[11px] uppercase tracking-[0.32em] text-muted-foreground">
                  Portfolio
                </div>
                <Badge className="border-accent/40 bg-accent/10 text-[10px] tracking-[0.18em] text-accent">
                  Live
                </Badge>
              </div>

              <div className="mt-4 flex flex-wrap items-end gap-x-4 gap-y-3">
                <div className="text-5xl font-semibold tracking-[-0.04em] text-foreground sm:text-6xl">
                  {usd(dashboardData?.wallet?.total_equity)}
                </div>
                <div
                  className={cn(
                    'rounded-full border px-3 py-1 text-sm font-semibold',
                    (dashboardData?.performance?.last_7d?.total_pnl ?? 0) >= 0
                      ? 'border-accent/35 bg-accent/10 text-accent'
                      : 'border-destructive/35 bg-destructive/10 text-destructive'
                  )}
                >
                  {usd(dashboardData?.performance?.last_7d?.total_pnl)} · 7d
                </div>
              </div>

              <div className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-2 text-sm">
                <div className="text-muted-foreground">
                  Profile{' '}
                  <span className="font-semibold text-foreground">
                    {dashboardData?.status?.trade_profile_label ||
                      dashboardData?.status?.trading_profile ||
                      'MEDIUM'}
                  </span>
                </div>
                <div className="text-muted-foreground">
                  Open{' '}
                  <span className="font-semibold text-violet-300">
                    {dashboardData?.positions?.items?.length || 0}
                  </span>
                </div>
                <div
                  className={cn(
                    'font-medium',
                    (dashboardData?.wallet?.unrealized_pnl ?? 0) >= 0
                      ? 'text-accent'
                      : 'text-destructive'
                  )}
                >
                  {usd(dashboardData?.wallet?.unrealized_pnl)} unrealized
                </div>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="relative overflow-hidden rounded-[20px] border border-border bg-secondary/40 p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                  Deposited
                </div>
                <div className="mt-3 text-[2rem] font-semibold tracking-[-0.03em] text-foreground">
                  {usd(dashboardData?.wallet?.wallet_balance)}
                </div>
                <div className="mt-2 text-xs text-muted-foreground">
                  {(dashboardData?.wallet?.margin_balance ?? 0) > 0
                    ? `${(((dashboardData?.wallet?.initial_margin || 0) / Math.max(1, dashboardData?.wallet?.margin_balance || 1)) * 100).toFixed(0)}% active`
                    : 'No active margin'}
                </div>
                <div className="absolute -right-5 -top-5 h-20 w-20 rounded-full border border-border" />
              </div>

              <div className="relative overflow-hidden rounded-[20px] border border-border bg-secondary/40 p-4">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Earnings
                    </div>
                    <div
                      className={cn(
                        'mt-3 text-[2rem] font-semibold tracking-[-0.03em]',
                        (dashboardData?.performance?.last_24h?.total_pnl ??
                          0) >= 0
                          ? 'text-accent'
                          : 'text-destructive'
                      )}
                    >
                      {usd(dashboardData?.performance?.last_24h?.total_pnl)}
                    </div>
                    <div className="mt-2 text-xs text-muted-foreground">
                      {dashboardData?.performance?.last_24h?.win_rate !==
                      undefined
                        ? `${dashboardData.performance.last_24h.win_rate.toFixed(1)}% win rate`
                        : '24h performance'}
                    </div>
                  </div>
                  <div className="text-xs text-muted-foreground">24h</div>
                </div>
                <div className="absolute bottom-0 right-0 h-14 w-20 rounded-tl-2xl bg-[linear-gradient(135deg,rgba(15,23,42,0)_0%,rgba(59,130,246,0.14)_100%)]" />
              </div>

              <div className="rounded-[20px] border border-accent/20 bg-accent/[0.08] p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                  Active Trading
                </div>
                <div className="mt-3 text-[2rem] font-semibold tracking-[-0.03em] text-foreground">
                  {usd(dashboardData?.wallet?.margin_balance)}
                </div>
                <div className="mt-2 flex items-center gap-2 text-xs text-accent">
                  <span className="h-2 w-2 rounded-full bg-accent" />
                  Working for you
                </div>
              </div>

              <div className="rounded-[20px] border border-primary/20 bg-primary/[0.06] p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                  Idle Funds
                </div>
                <div className="mt-3 text-[2rem] font-semibold tracking-[-0.03em] text-primary">
                  {usd(dashboardData?.wallet?.available_balance)}
                </div>
                <div className="mt-2 text-xs text-primary/80">
                  Ready capital
                </div>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Strategy Cockpit — live consensus & entry-guard state */}
        <StrategyCockpit />

        {/* Positions */}
        <PositionTable
          positions={openPositions}
          guardedSetups={guardedSetups}
          marketSignals={
            dashboardData?.market_signals?.items
              ? Object.values(dashboardData.market_signals.items as any)
              : []
          }
        />

        {/* Recent fills — live feed */}
        <RecentFills trades={closedTrades} />

        {/* Closed Trades */}
        <ClosedTradesTable trades={closedTrades} />

        {/* Architecture Flow */}
        <Card className="border-0 bg-transparent shadow-none [&>*]:px-0">
          <CardHeader className="pb-1">
            <CardTitle className="text-base text-foreground">
              Architecture Flow
            </CardTitle>
          </CardHeader>
          <CardContent className="pt-0">
            <ServiceFlowDiagram
              services={dashboardData?.services || []}
              executionMode={tradingMode}
              executorState={executorEnabled ? 'running' : 'down'}
              flowContext={flowContext}
              activeSignalsCount={Object.keys(effectiveMarketSignals).length}
              openPositionsCount={dashboardData?.positions?.items?.length || 0}
              closedTradesCount={
                (dashboardData?.trades?.items || []).filter(
                  trade => trade.status === 'closed'
                ).length
              }
            />
          </CardContent>
        </Card>
      </main>
    </div>
  );
}
