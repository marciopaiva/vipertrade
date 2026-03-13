"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties, ReactNode } from "react";

type DashboardPayload = {
  source: { baseUrl: string; fetchedAt: string };
  status: {
    service?: string;
    risk_status?: string;
    trading_mode?: string;
    trading_profile?: string;
    db_connected?: boolean;
    operator_controls_enabled?: boolean;
    critical_reconciliation_events_15m?: number;
    kill_switch?: {
      enabled?: boolean;
      reason?: string | null;
      actor?: string | null;
      updated_at?: string | null;
    };
    executor?: {
      enabled?: boolean;
      reason?: string | null;
      actor?: string | null;
      updated_at?: string | null;
    };
    risk_limits?: {
      max_daily_loss_pct?: number;
      max_leverage?: number;
      risk_per_trade_pct?: number;
    };
  };
  performance: {
    last_24h?: { total_trades?: number; total_pnl?: number; win_rate?: number };
    last_7d?: { total_trades?: number; total_pnl?: number; win_rate?: number };
    last_30d?: { total_trades?: number; total_pnl?: number; win_rate?: number };
    max_drawdown_30d?: number | null;
  };
  positions: {
    items?: Array<{
      trade_id: string;
      symbol: string;
      side: string;
      quantity: number;
      notional_usdt: number;
      entry_price: number;
      trailing_stop_activated?: boolean;
      trailing_stop_peak_price?: number | null;
      trailing_stop_final_distance_pct?: number | null;
      stop_loss_price?: number | null;
      trailing_activation_price?: number | null;
      fixed_take_profit_price?: number | null;
      break_even_price?: number | null;
    }>;
  };
  trades: {
    items?: Array<{
      trade_id: string;
      symbol: string;
      side: string;
      status: string;
      quantity: number;
      entry_price: number;
      exit_price?: number | null;
      pnl?: number | null;
      close_reason?: string | null;
      opened_at: string;
      closed_at?: string | null;
    }>;
  };
  daily_trades_summary?: {
    ok?: boolean;
    source?: string;
    count?: number;
    window_start_utc?: string;
    window_end_utc?: string;
    checked_at?: string;
    status?: number;
    url?: string;
    error?: string | null;
    ret_code?: number | null;
    ret_msg?: string | null;
  };
  events: {
    items?: Array<{
      event_id: string;
      event_type: string;
      severity: string;
      category?: string | null;
      symbol?: string | null;
      data?: Record<string, unknown>;
      timestamp: string;
    }>;
  };
  market_signals?: {
    updated_at?: string | null;
    items?: Record<string, {
      symbol: string;
      current_price: number;
      bybit_price?: number;
      atr_14: number;
      volume_24h: number;
      funding_rate: number;
      trend_score: number;
      spread_pct: number;
    }> | Array<{
      symbol: string;
      current_price: number;
      bybit_price?: number;
      atr_14: number;
      volume_24h: number;
      funding_rate: number;
      trend_score: number;
      spread_pct: number;
    }>;
  };
  analytics_scores?: {
    updated_at?: string;
    horizon_minutes?: number;
    lookback_hours?: number;
    exchanges?: Array<{
      exchange: string;
      evaluated: number;
      hits: number;
      hit_rate: number;
      avg_forward_return: number;
    }>;
    by_symbol?: Array<{
      exchange: string;
      symbol: string;
      evaluated: number;
      hits: number;
      hit_rate: number;
      avg_forward_return: number;
    }>;
  };
  wallet?: {
    ok?: boolean;
    status?: number;
    url?: string;
    error?: string | null;
    ret_code?: number | null;
    ret_msg?: string | null;
    checked_at?: string;
    account_type?: string;
    total_equity?: number | null;
    wallet_balance?: number | null;
    margin_balance?: number | null;
    available_balance?: number | null;
    unrealized_pnl?: number | null;
    initial_margin?: number | null;
    maintenance_margin?: number | null;
    account_im_rate?: number | null;
    account_mm_rate?: number | null;
  };
  risk_kpis: {
    rejected_orders_24h?: number;
    open_exposure_usdt?: number;
    realized_pnl_24h?: number;
    critical_events_24h?: number;
    circuit_breaker_triggers_24h?: number;
  };
  control_state: {
    operator_auth_mode?: string;
    operator_controls_enabled?: boolean;
    kill_switch?: {
      enabled?: boolean;
      reason?: string | null;
      actor?: string | null;
      updated_at?: string | null;
    };
    executor?: {
      enabled?: boolean;
      reason?: string | null;
      actor?: string | null;
      updated_at?: string | null;
    };
    risk_limits?: {
      max_daily_loss_pct?: number;
      max_leverage?: number;
      risk_per_trade_pct?: number;
      reason?: string | null;
      actor?: string | null;
      updated_at?: string | null;
    };
  };
  services: Array<{
    name: string;
    ok: boolean;
    status: number;
    latency_ms: number;
    url: string;
    error?: string;
  }>;
  partial?: boolean;
  warnings?: string[];
};

type ControlKind = "kill-switch" | "executor" | "risk-limits";
type SignalBucket = { buy: number; hold: number; sell: number; total: number };
type HoldReasonStat = { reason: string; count: number };
type TokenSignalSummary = SignalBucket & { symbol: string; holdReasons: HoldReasonStat[] };
type TokenDecisionCardData = {
  symbol: string;
  regime: string;
  bybitRegime: string;
  consensusCount: number;
  exchangesAvailable: number;
  trendScore: number;
  stateLabel: string;
  stateTone: "positive" | "negative" | "neutral";
  recentBuy: number;
  recentHold: number;
  recentSell: number;
  mainBlock: string;
  priorityRank: number;
};
type RealtimeMarketSignal = {
  symbol: string;
  current_price: number;
  bybit_price?: number;
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
};

function num(value: number | null | undefined, digits = 2) {
  if (typeof value !== "number" || Number.isNaN(value)) return "-";
  return value.toFixed(digits);
}

function pct(value: number | null | undefined) {
  if (typeof value !== "number" || Number.isNaN(value)) return "-";
  return `${(value * 100).toFixed(2)}%`;
}

function humanVolume(value: number | null | undefined) {
  if (typeof value !== "number" || Number.isNaN(value)) return "-";
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(2)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(2)}K`;
  return value.toFixed(0);
}

function prettifyReason(reason: string | null | undefined) {
  if (!reason) return "No clear block";
  return reason
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

function regimeTone(regime: string | null | undefined) {
  if (regime === "bullish") return { label: "Bullish", ok: true };
  if (regime === "bearish") return { label: "Bearish", ok: false };
  return { label: "Neutral", ok: undefined };
}

function usd(value: number | null | undefined) {
  if (typeof value !== "number" || Number.isNaN(value)) return "-";
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }).format(value);
}

function usdAdaptive(value: number | null | undefined) {
  if (typeof value !== "number" || Number.isNaN(value)) return "-";
  const digits = Math.abs(value) < 0.1 ? 4 : 2;
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

function statusTone(ok: boolean): CSSProperties {
  return {
    color: ok ? "#38f9a5" : "#ff8f8f",
    borderColor: ok ? "rgba(56, 249, 165, 0.55)" : "rgba(255, 143, 143, 0.5)",
    background: ok ? "rgba(4, 89, 63, 0.28)" : "rgba(107, 16, 16, 0.3)",
  };
}

function serviceColor(name: string, ok: boolean): string {
  if (!ok) return "#ff6478";
  if (name.includes("bybit")) return "#f7b500";
  if (name === "api") return "#11c4ff";
  if (name === "executor") return "#00e5a8";
  if (name === "analytics") return "#7bc4ff";
  if (name === "strategy") return "#9580ff";
  return "#46d8ff";
}

export default function Home() {
  const [data, setData] = useState<DashboardPayload | null>(null);
  const [error, setError] = useState<string>("");
  const [loading, setLoading] = useState(true);

  const [operatorToken, setOperatorToken] = useState("");
  const [operatorId, setOperatorId] = useState("web-operator");
  const [actionReason, setActionReason] = useState("manual_web_action");
  const [controlMessage, setControlMessage] = useState("");
  const [controlBusy, setControlBusy] = useState<ControlKind | "">("");

  const [maxDailyLossPct, setMaxDailyLossPct] = useState<string>("3");
  const [maxLeverage, setMaxLeverage] = useState<string>("2");
  const [riskPerTradePct, setRiskPerTradePct] = useState<string>("1.25");

  const fetchDashboard = useCallback(async () => {
    try {
      const res = await fetch("/api/dashboard", { cache: "no-store" });
      const raw = await res.text();
      let body: (DashboardPayload & { message?: string }) | null = null;
      try {
        body = raw ? (JSON.parse(raw) as DashboardPayload & { message?: string }) : null;
      } catch {
        throw new Error(`dashboard response is not JSON (http ${res.status})`);
      }
      if (!res.ok) {
        throw new Error(body?.message || `failed: ${res.status}`);
      }
      if (!body) {
        throw new Error(`empty dashboard response (http ${res.status})`);
      }
      setData(body);
      setError("");

      const limits = body.control_state?.risk_limits;
      if (limits) {
        if (typeof limits.max_daily_loss_pct === "number") {
          setMaxDailyLossPct(String(limits.max_daily_loss_pct));
        }
        if (typeof limits.max_leverage === "number") {
          setMaxLeverage(String(limits.max_leverage));
        }
        if (typeof limits.risk_per_trade_pct === "number") {
          setRiskPerTradePct(String(limits.risk_per_trade_pct));
        }
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchDashboard();
    const interval = setInterval(() => void fetchDashboard(), 5000);
    return () => clearInterval(interval);
  }, [fetchDashboard]);

  const positions = useMemo(() => data?.positions?.items ?? [], [data]);
  const trades = useMemo(() => data?.trades?.items ?? [], [data]);
  const events = useMemo(() => data?.events?.items ?? [], [data]);
  const services = useMemo(() => data?.services ?? [], [data]);
  const closedTrades = useMemo(() => {
    const sevenDaysAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
    return trades.filter((t) => {
      if (t.status !== "closed") return false;
      const referenceTime = Date.parse(t.closed_at || t.opened_at);
      return Number.isFinite(referenceTime) && referenceTime >= sevenDaysAgo;
    });
  }, [trades]);
  const tradeStats = useMemo(() => {
    const avgClosedPnlPct =
      closedTrades.length > 0
        ? closedTrades.reduce((acc, t) => {
            const notional = t.quantity * t.entry_price;
            if (!notional) return acc;
            return acc + ((t.pnl ?? 0) / notional);
          }, 0) / closedTrades.length
        : null;
    return {
      winRate24h: data?.performance?.last_24h?.win_rate,
      winRate7d: data?.performance?.last_7d?.win_rate,
      pnl24h: data?.performance?.last_24h?.total_pnl,
      totalTrades: data?.performance?.last_7d?.total_trades ?? closedTrades.length,
      avgClosedPnlPct,
    };
  }, [closedTrades, data]);

  const executionMode = (() => {
    const mode = String(data?.status?.trading_mode || "").toLowerCase();
    if (mode === "testnet") return "testnet" as const;
    if (mode === "mainnet" || mode === "live") return "mainnet" as const;
    return "paper" as const;
  })();
  const flowOrder = useMemo(
    () =>
      executionMode === "testnet"
        ? ["bybit", "market-data", "strategy", "executor", "api", "monitor", "analytics", "backtest"]
        : ["bybit", "binance", "okx", "market-data", "strategy", "executor", "api", "monitor", "analytics", "backtest"],
    [executionMode],
  );
  const flowServices = useMemo(
    () => flowOrder.map((name) => services.find((svc) => svc.name === name)).filter(Boolean) as DashboardPayload["services"],
    [flowOrder, services],
  );

  const executorServiceOk = useMemo(() => {
    const svc = services.find((item) => item.name === "executor");
    return !!svc?.ok;
  }, [services]);
  const bybitPrivateOk = useMemo(() => {
    const svc = services.find((item) => item.name === "bybit-private");
    return !!svc?.ok;
  }, [services]);
  const redisConnected = useMemo(() => {
    const apiSvc = services.find((item) => item.name === "api");
    return !!apiSvc?.ok;
  }, [services]);
  const executorControlEnabled = !!data?.control_state?.executor?.enabled;
  const killSwitchEnabled = !!data?.control_state?.kill_switch?.enabled;
  const executorVisualState: "running" | "paused" | "down" =
    !executorServiceOk ? "down" : executorControlEnabled ? "running" : "paused";

  const tokenSignalStats = useMemo<TokenSignalSummary[]>(() => {
    const defaultTokens = ["DOGEUSDT", "XRPUSDT", "ADAUSDT", "XLMUSDT"];
    const stats = new Map<string, SignalBucket>();
    const holdReasons = new Map<string, Map<string, number>>();
    for (const token of defaultTokens) {
      stats.set(token, { buy: 0, hold: 0, sell: 0, total: 0 });
      holdReasons.set(token, new Map<string, number>());
    }

    for (const evt of events) {
      if (evt.event_type !== "executor_event_processed") continue;
      const symbolRaw =
        evt.symbol ||
        (typeof evt.data?.symbol === "string" ? evt.data.symbol : null) ||
        (typeof evt.data?.decision_symbol === "string" ? evt.data.decision_symbol : null);
      if (!symbolRaw) continue;
      const symbol = String(symbolRaw).toUpperCase();

      const actionRaw = typeof evt.data?.action === "string" ? evt.data.action.toUpperCase() : "";
      let bucket: keyof SignalBucket | null = null;
      if (actionRaw === "HOLD") bucket = "hold";
      else if (actionRaw === "ENTER_LONG" || actionRaw === "ENTER_SHORT" || actionRaw === "BUY") bucket = "buy";
      else if (actionRaw === "CLOSE_LONG" || actionRaw === "CLOSE_SHORT" || actionRaw === "SELL") bucket = "sell";
      if (!bucket) continue;

      const item = stats.get(symbol) ?? { buy: 0, hold: 0, sell: 0, total: 0 };
      item[bucket] += 1;
      item.total += 1;
      stats.set(symbol, item);

      if (bucket === "hold") {
        const reasonRaw =
          (typeof evt.data?.reason === "string" && evt.data.reason) ||
          (typeof evt.data?.decision_reason === "string" && evt.data.decision_reason) ||
          (typeof evt.data?.message === "string" && evt.data.message) ||
          "hold_without_reason";
        const symbolReasons = holdReasons.get(symbol) ?? new Map<string, number>();
        symbolReasons.set(reasonRaw, (symbolReasons.get(reasonRaw) ?? 0) + 1);
        holdReasons.set(symbol, symbolReasons);
      }
    }

    return Array.from(stats.entries())
      .sort((a, b) => b[1].total - a[1].total || a[0].localeCompare(b[0]))
      .map(([symbol, s]) => ({
        symbol,
        ...s,
        holdReasons: Array.from((holdReasons.get(symbol) ?? new Map<string, number>()).entries())
          .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
          .slice(0, 3)
          .map(([reason, count]) => ({ reason, count })),
      }));
  }, [events]);
  const realtimeMarketSignals = useMemo<RealtimeMarketSignal[]>(() => {
    const raw = data?.market_signals?.items;
    if (!raw) return [];

    const list = Array.isArray(raw) ? raw : Object.values(raw);
    return list
      .filter((item): item is RealtimeMarketSignal => typeof item?.symbol === "string")
      .sort((a, b) => a.symbol.localeCompare(b.symbol));
  }, [data?.market_signals?.items]);
  const analyticsExchanges = useMemo(
    () =>
      (data?.analytics_scores?.exchanges ?? [])
        .slice()
        .sort((a, b) => (b.hit_rate ?? 0) - (a.hit_rate ?? 0)),
    [data?.analytics_scores?.exchanges],
  );
  const openPositionsView = useMemo(() => {
    const marketBySymbol = new Map(realtimeMarketSignals.map((s) => [s.symbol, s]));
    return positions.map((p) => {
      const entryPrice = p.entry_price || (p.quantity > 0 ? p.notional_usdt / p.quantity : 0);
      const signal = marketBySymbol.get(p.symbol);
      const markPrice = signal?.bybit_price ?? signal?.current_price ?? null;
      const isLong = p.side.toLowerCase() === "long";
      const unrealizedPnl =
        typeof markPrice === "number"
          ? (isLong ? markPrice - entryPrice : entryPrice - markPrice) * p.quantity
          : null;
      const unrealizedPnlPct = p.notional_usdt > 0 && typeof unrealizedPnl === "number" ? unrealizedPnl / p.notional_usdt : null;
      const markDeltaPct =
        typeof markPrice === "number" && entryPrice > 0
          ? (isLong ? (markPrice - entryPrice) / entryPrice : (entryPrice - markPrice) / entryPrice)
          : null;
      const trailingLivePrice =
        p.trailing_stop_activated &&
        typeof p.trailing_stop_peak_price === "number" &&
        typeof p.trailing_stop_final_distance_pct === "number"
          ? isLong
            ? p.trailing_stop_peak_price * (1 - p.trailing_stop_final_distance_pct)
            : p.trailing_stop_peak_price * (1 + p.trailing_stop_final_distance_pct)
          : null;
      const triggerState =
        p.trailing_stop_activated && typeof trailingLivePrice === "number"
          ? `Trail ${num(trailingLivePrice, 6)}`
          : typeof p.trailing_activation_price === "number"
            ? `Arm ${num(p.trailing_activation_price, 6)}`
            : typeof p.fixed_take_profit_price === "number"
              ? `TP ${num(p.fixed_take_profit_price, 6)}`
              : "-";

      return {
        ...p,
        entryPrice,
        markPrice,
        unrealizedPnl,
        unrealizedPnlPct,
        markDeltaPct,
        trailingLivePrice,
        triggerState,
      };
    });
  }, [positions, realtimeMarketSignals]);
  const tokenDecisionBoard = useMemo<TokenDecisionCardData[]>(() => {
    const statsBySymbol = new Map(tokenSignalStats.map((item) => [item.symbol, item]));
    return realtimeMarketSignals
      .map((signal) => {
        const stats = statsBySymbol.get(signal.symbol);
      const holdBlock = stats?.holdReasons?.[0]?.reason;
      const consensus = signal.consensus_count ?? 0;
      const exchanges = signal.exchanges_available ?? 0;
      let stateLabel = "Watching";
      let stateTone: TokenDecisionCardData["stateTone"] = "neutral";
      let priorityRank = 2;

      if (signal.regime === "bullish" && consensus >= 2) {
        stateLabel = "Ready Long";
        stateTone = "positive";
        priorityRank = 0;
      } else if (signal.regime === "bearish" && consensus >= 2) {
        stateLabel = "Ready Short";
        stateTone = "negative";
        priorityRank = 0;
      } else if (holdBlock) {
        stateLabel = "Blocked";
        stateTone = "neutral";
        priorityRank = 3;
      }

      return {
        symbol: signal.symbol,
        regime: signal.regime ?? "neutral",
        bybitRegime: signal.bybit_regime ?? "neutral",
        consensusCount: consensus,
        exchangesAvailable: exchanges,
        trendScore: signal.trend_score,
        stateLabel,
        stateTone,
        recentBuy: stats?.buy ?? 0,
        recentHold: stats?.hold ?? 0,
        recentSell: stats?.sell ?? 0,
        mainBlock: prettifyReason(holdBlock ?? "no clear block"),
        priorityRank,
      };
      })
      .sort(
        (a, b) =>
          a.priorityRank - b.priorityRank ||
          b.consensusCount - a.consensusCount ||
          Math.abs(b.trendScore) - Math.abs(a.trendScore) ||
          a.symbol.localeCompare(b.symbol),
      );
  }, [realtimeMarketSignals, tokenSignalStats]);
  const exchangeLeader = analyticsExchanges[0] ?? null;
  const wallet = data?.wallet;
  const walletCards = useMemo(
    () => [
      {
        label: "Total Equity",
        value: usd(wallet?.total_equity),
        accent: "#11c4ff",
        helper: `${wallet?.account_type || "UNIFIED"} account`,
      },
      {
        label: "Margin Balance",
        value: usd(wallet?.margin_balance),
        accent: "#90dcff",
        helper:
          typeof wallet?.available_balance === "number"
            ? `Available ${usd(wallet.available_balance)}`
            : "Available balance unavailable",
      },
      {
        label: "Unrealized PnL",
        value: usd(wallet?.unrealized_pnl),
        accent: (wallet?.unrealized_pnl ?? 0) >= 0 ? "#00e5a8" : "#ff6478",
        helper:
          typeof wallet?.wallet_balance === "number"
            ? `Wallet ${usd(wallet.wallet_balance)}`
            : "Wallet balance unavailable",
      },
    ],
    [wallet],
  );

  const sendControl = useCallback(
    async (kind: ControlKind, payload: Record<string, unknown>) => {
      setControlBusy(kind);
      setControlMessage("");
      try {
        const res = await fetch("/api/control", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            kind,
            payload,
            operatorToken,
            operatorId,
          }),
        });
        const raw = await res.text();
        let body: { message?: string; error?: string; source?: string } | null = null;
        try {
          body = raw ? (JSON.parse(raw) as { message?: string; error?: string; source?: string }) : null;
        } catch {
          throw new Error(`control response is not JSON (http ${res.status})`);
        }
        if (!res.ok) {
          throw new Error(body?.message || body?.error || `failed: ${res.status}`);
        }
        setControlMessage(`Command '${kind}' applied via ${body?.source || "backend"}`);
        await fetchDashboard();
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setControlMessage(`Command '${kind}' failed: ${message}`);
      } finally {
        setControlBusy("");
      }
    },
    [fetchDashboard, operatorId, operatorToken],
  );

  return (
    <main style={shellStyle}>
        <section style={heroStyle}>
          <div>
            <div>
              <h1 style={{ margin: 0, fontSize: "clamp(2rem, 6vw, 3rem)", letterSpacing: 0.2 }}>ViperTrade Control Center</h1>
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 12 }}>
                <span
                  style={{
                    ...miniBadgeStyle,
                    ...(executionMode === "mainnet"
                      ? { borderColor: "rgba(255,100,120,0.5)", color: "#ff9aa8", background: "rgba(96,16,30,0.22)" }
                      : executionMode === "testnet"
                        ? { borderColor: "rgba(247,181,0,0.45)", color: "#ffd978", background: "rgba(108,78,8,0.22)" }
                        : statusTone(true)),
                  }}
                >
                  {executionMode === "mainnet" ? "MAINNET MODE" : executionMode === "testnet" ? "TESTNET MODE" : "PAPER MODE"}
                </span>
                <span
                  style={{
                    ...miniBadgeStyle,
                    ...(executorVisualState === "running"
                      ? statusTone(true)
                      : executorVisualState === "paused"
                        ? { borderColor: "rgba(247,181,0,0.45)", color: "#ffd978", background: "rgba(108,78,8,0.22)" }
                        : statusTone(false)),
                  }}
                >
                  Executor {executorVisualState.toUpperCase()}
                </span>
                <span
                  style={{
                    ...miniBadgeStyle,
                    ...(killSwitchEnabled
                      ? statusTone(false)
                      : { borderColor: "rgba(56,249,165,0.38)", color: "#8cf7c6", background: "rgba(8,73,49,0.18)" }),
                  }}
                >
                  Kill Switch {killSwitchEnabled ? "ON" : "OFF"}
                </span>
              </div>
            </div>
          </div>

          {data && (
            <div style={heroWalletSectionStyle}>
              <div style={walletTopStripStyle}>
                <WalletRateCard
                  label="IM"
                  amount={wallet?.initial_margin}
                  rate={wallet?.account_im_rate}
                  tone={wallet?.ok ? "positive" : "neutral"}
                />
                <WalletRateCard
                  label="MM"
                  amount={wallet?.maintenance_margin}
                  rate={wallet?.account_mm_rate}
                  tone={wallet?.ok ? "positive" : "neutral"}
                />
              </div>

              <div style={heroGridStyle}>
                {walletCards.map((item) => (
                  <MetricCard key={item.label} label={item.label} value={item.value} accent={item.accent} helper={item.helper} />
                ))}
              </div>

              <div style={heroGridStyle}>
                <MetricCard label="Open Positions" value={String(positions.length)} accent="#9e8bff" />
                <MetricCard
                  label="Trades Today"
                  value={String(data?.daily_trades_summary?.count ?? 0)}
                  accent="#f7b500"
                />
              </div>

              {!wallet?.ok && (
                <div style={{ marginTop: 12, color: "#ff9aa8", fontSize: 13 }}>
                  {wallet?.error || wallet?.ret_msg || "Wallet snapshot unavailable."}
                </div>
              )}

              <div style={heroOpsSectionStyle}>
                  <div style={splitTwoStyle}>
                  <div style={panelBoxStyle}>
                    <ServiceFlowGraph services={flowServices} executionMode={executionMode} executorVisualState={executorVisualState} />
                  </div>

                  <div style={{ display: "grid", gap: 12 }}>
                    <div style={statusPillGridStyle}>
                      <StatusPill
                        label="Critical Recon (15m)"
                        value={String(data.status?.critical_reconciliation_events_15m ?? "-")}
                        ok={(data.status?.critical_reconciliation_events_15m ?? 0) === 0}
                        icon="recon"
                      />
                      <StatusPill label="DB" value={data.status?.db_connected ? "connected" : "disconnected"} ok={!!data.status?.db_connected} icon="db" />
                      <StatusPill label="Redis" value={redisConnected ? "connected" : "unknown"} ok={redisConnected} icon="redis" />
                      <StatusPill label="Executor" value={executorVisualState} ok={executorVisualState === "running"} icon="executor" />
                      <StatusPill label="Bybit Private" value={bybitPrivateOk ? "ok" : "down"} ok={bybitPrivateOk} icon="bybit" />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}
        </section>

        {loading && <Panel title="Loading">Loading dashboard...</Panel>}
        {!!error && <Panel title="Erro" tone="danger">{error}</Panel>}
        {!!data?.partial && (
          <Panel title="Partial data mode" tone="warn">
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {(data.warnings ?? []).map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          </Panel>
        )}

        {data && (
          <>
            <section style={panelBoxStyle}>
              <h2 style={h2Style}>Token Decision Board</h2>
              <div style={sectionDividerStyle} />
              {tokenDecisionBoard.length === 0 ? (
                <p style={mutedStyle}>No token decision data available.</p>
              ) : (
                <div style={tokenGridStyle}>
                  {tokenDecisionBoard.map((item) => (
                    <TokenDecisionBoardCard key={item.symbol} item={item} />
                  ))}
                </div>
              )}
            </section>

            <section style={panelBoxStyle}>
              <h2 style={h2Style}>Exchange Accuracy (Historical)</h2>
              <div style={sectionDividerStyle} />
              {analyticsExchanges.length === 0 ? (
                <p style={mutedStyle}>No exchange score data yet.</p>
              ) : (
                <div style={exchangeRankGridStyle}>
                  {analyticsExchanges.map((row) => (
                    <article
                      key={row.exchange}
                      style={{
                        ...exchangeRankCardStyle,
                        borderColor:
                          exchangeLeader?.exchange === row.exchange ? "rgba(56,249,165,0.45)" : "rgba(95,137,203,0.24)",
                      }}
                    >
                      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                          <strong style={{ fontSize: 15 }}>{row.exchange.toUpperCase()}</strong>
                          {exchangeLeader?.exchange === row.exchange && (
                            <span style={{ ...miniBadgeStyle, ...statusTone(true) }}>leader</span>
                          )}
                        </div>
                        <span style={{ color: "#91abd6", fontSize: 12 }}>{row.evaluated} samples</span>
                      </div>
                      <div style={exchangeMetricsRowStyle}>
                        <div>
                          <div style={exchangeMetricLabelStyle}>Hit Rate</div>
                          <div style={{ ...exchangeMetricValueStyle, color: "#90dcff" }}>{pct(row.hit_rate)}</div>
                        </div>
                        <div>
                          <div style={exchangeMetricLabelStyle}>Avg Return</div>
                          <div
                            style={{
                              ...exchangeMetricValueStyle,
                              color: row.avg_forward_return >= 0 ? "#38f9a5" : "#ff8f8f",
                            }}
                          >
                            {pct(row.avg_forward_return)}
                          </div>
                        </div>
                      </div>
                      <div style={exchangeBarTrackStyle}>
                        <div
                          style={{
                            ...exchangeBarFillStyle,
                            width: `${Math.max(8, Math.min(100, (row.hit_rate ?? 0) * 100))}%`,
                            background:
                              exchangeLeader?.exchange === row.exchange
                                ? "linear-gradient(90deg, rgba(56,249,165,0.78), rgba(116,255,210,0.96))"
                                : "linear-gradient(90deg, rgba(70,174,255,0.72), rgba(144,220,255,0.92))",
                          }}
                        />
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>

            <section style={stackCardsStyle}>
              <div style={panelBoxStyle}>
                <h2 style={h2Style}>Open Positions</h2>
                <div style={sectionDividerStyle} />
                {openPositionsView.length === 0 ? (
                  <p style={mutedStyle}>No open positions.</p>
                ) : (
                  <div style={positionListStyle}>
                    {openPositionsView.map((p) => {
                      const sideLong = p.side.toLowerCase() === "long";
                      const pnlColor = (p.unrealizedPnl ?? 0) >= 0 ? "#38f9a5" : "#ff8f8f";
                      return (
                        <article key={`${p.symbol}-${p.side}`} style={positionRowStyle}>
                          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                            <strong style={{ minWidth: 86 }}>{p.symbol}</strong>
                            <span style={{ color: sideLong ? "#38f9a5" : "#ff8f8f", fontWeight: 700, fontSize: 13 }}>
                              {sideLong ? "LONG" : "SHORT"}
                            </span>
                          </div>
                          <div style={{ textAlign: "right" }}>
                            <div style={positionLabelStyle}>Qty</div>
                            <div>{num(p.quantity, 6)}</div>
                          </div>
                          <div style={{ textAlign: "right" }}>
                            <div style={positionLabelStyle}>Entry</div>
                            <div>{num(p.entryPrice, 6)}</div>
                          </div>
                          <div style={{ textAlign: "right" }}>
                            <div style={positionLabelStyle}>Bybit / Delta</div>
                            <div>{typeof p.markPrice === "number" ? num(p.markPrice, 6) : "-"}</div>
                            <div style={{ color: "#8fb7e8", fontSize: 12 }}>
                              {typeof p.markDeltaPct === "number" ? pct(p.markDeltaPct) : "-"}
                            </div>
                          </div>
                          <div style={{ textAlign: "right" }}>
                            <div style={positionLabelStyle}>Trigger</div>
                            <div style={{ color: p.trailing_stop_activated ? "#38f9a5" : "#90dcff", fontWeight: 700 }}>
                              {p.triggerState}
                            </div>
                          </div>
                          <div style={{ textAlign: "right" }}>
                            <div style={positionLabelStyle}>PnL</div>
                            <div style={{ color: pnlColor, fontWeight: 700 }}>
                              {typeof p.unrealizedPnl === "number" ? usdAdaptive(p.unrealizedPnl) : "-"}
                              {typeof p.unrealizedPnlPct === "number" ? ` (${pct(p.unrealizedPnlPct)})` : ""}
                            </div>
                          </div>
                        </article>
                      );
                    })}
                  </div>
                )}
              </div>

              <div style={panelBoxStyle}>
                <h2 style={h2Style}>Recent Closed Trades</h2>
                <div style={sectionDividerStyle} />
                {closedTrades.length === 0 ? (
                  <p style={mutedStyle}>No closed trades yet. PnL appears after position close.</p>
                ) : (
                  <>
                    <div style={tradeStatsGridStyle}>
                      <article style={tradeStatCardStyle}>
                        <div style={tradeStatLabelStyle}>24h Win Rate</div>
                        <div style={tradeStatValueStyle}>{pct(tradeStats.winRate24h)}</div>
                      </article>
                      <article style={tradeStatCardStyle}>
                        <div style={tradeStatLabelStyle}>7d Win Rate</div>
                        <div style={tradeStatValueStyle}>{pct(tradeStats.winRate7d)}</div>
                      </article>
                      <article style={{ ...tradeStatCardStyle, borderColor: "rgba(56,249,165,0.48)", background: "rgba(10, 52, 45, 0.28)" }}>
                        <div style={tradeStatLabelStyle}>24h PnL</div>
                        <div style={{ ...tradeStatValueStyle, color: (tradeStats.pnl24h ?? 0) >= 0 ? "#38f9a5" : "#ff8f8f" }}>
                          {usd(tradeStats.pnl24h)}
                        </div>
                      </article>
                      <article style={tradeStatCardStyle}>
                        <div style={tradeStatLabelStyle}>Total Trades</div>
                        <div style={tradeStatValueStyle}>{String(tradeStats.totalTrades ?? "-")}</div>
                      </article>
                    </div>

                    <div style={tradeListStyle}>
                      {closedTrades.map((t) => {
                        const sideLong = t.side.toUpperCase() === "LONG";
                        const notional = t.quantity * t.entry_price;
                        const pnlPct = notional > 0 ? (t.pnl ?? 0) / notional : null;
                        const refTs = t.closed_at || t.opened_at;
                        const soldAt = new Date(refTs);
                        return (
                          <article key={t.trade_id} style={tradeRowStyle}>
                            <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                              <strong style={{ minWidth: 74 }}>{t.symbol.replace("USDT", "")}</strong>
                              <span style={{ color: sideLong ? "#38f9a5" : "#ff8f8f", fontWeight: 700, fontSize: 13 }}>
                                {sideLong ? "LONG" : "SHORT"}
                              </span>
                            </div>
                            <div style={{ textAlign: "right" }}>
                              <div style={{ color: "#9fb6dc" }}>
                                {num(t.entry_price, 4)} {typeof t.exit_price === "number" ? `→ ${num(t.exit_price, 4)}` : "→ -"}
                              </div>
                              <div style={{ color: "#7088b6", fontSize: 12 }}>{usd(notional)}</div>
                            </div>
                            <div style={{ color: (pnlPct ?? 0) >= 0 ? "#38f9a5" : "#ff8f8f", textAlign: "right", fontWeight: 700 }}>
                              {usdAdaptive(t.pnl ?? 0)}
                              <div style={{ fontSize: 12 }}>{pct(pnlPct)}</div>
                            </div>
                            <div style={{ textAlign: "right" }}>
                              <div style={{ color: "#9fb6dc", fontSize: 12 }}>Sold</div>
                              <div style={{ color: "#7f95bd" }}>
                                {soldAt.toLocaleDateString()} {soldAt.toLocaleTimeString()}
                              </div>
                            </div>
                          </article>
                        );
                      })}
                    </div>
                  </>
                )}
              </div>
            </section>

          </>
        )}
    </main>
  );
}

function Panel({ title, children, tone = "normal" }: { title: string; children: ReactNode; tone?: "normal" | "warn" | "danger" }) {
  const toneStyle: CSSProperties =
    tone === "warn"
      ? { borderColor: "rgba(247, 181, 0, 0.6)", background: "rgba(105, 79, 4, 0.25)", color: "#ffd978" }
      : tone === "danger"
        ? { borderColor: "rgba(255, 100, 120, 0.6)", background: "rgba(96, 16, 30, 0.28)", color: "#ff9aa8" }
        : {};
  return (
    <section style={{ ...panelBoxStyle, ...toneStyle }}>
      <h2 style={{ ...h2Style, marginBottom: 8 }}>{title}</h2>
      <div>{children}</div>
    </section>
  );
}

function MetricCard({ label, value, accent, helper }: { label: string; value: string; accent: string; helper?: string }) {
  return (
    <article
      style={{
        border: `1px solid ${accent}55`,
        borderRadius: 14,
        padding: 14,
        background: "linear-gradient(180deg, rgba(4,20,43,0.85), rgba(6,14,27,0.9))",
        boxShadow: `inset 0 0 0 1px ${accent}22`,
      }}
    >
      <div style={{ fontSize: 12, letterSpacing: 0.4, textTransform: "uppercase", color: "#86a7dc", marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: 28, fontWeight: 700, color: accent }}>{value}</div>
      {helper && <div style={{ marginTop: 6, fontSize: 12, color: "#89a8d5" }}>{helper}</div>}
    </article>
  );
}

function WalletRateCard({
  label,
  amount,
  rate,
  tone,
}: {
  label: string;
  amount: number | null | undefined;
  rate: number | null | undefined;
  tone: "positive" | "neutral";
}) {
  const color = tone === "positive" ? "#38f9a5" : "#9fb6dc";
  return (
    <article style={walletRateCardStyle}>
      <div style={{ fontSize: 13, color: "#90a9d4", minWidth: 22 }}>{label}</div>
      <div style={{ flex: 1, height: 8, borderRadius: 999, background: "rgba(255,255,255,0.06)", overflow: "hidden" }}>
        <div
          style={{
            width: `${Math.min(Math.max(((rate ?? 0) * 100) / 5, 8), 100)}%`,
            height: "100%",
            borderRadius: 999,
            background: `linear-gradient(90deg, ${color}55, ${color})`,
          }}
        />
      </div>
      <div style={{ minWidth: 154, display: "flex", justifyContent: "flex-end", gap: 8, fontVariantNumeric: "tabular-nums" }}>
        <span style={{ color, fontWeight: 700 }}>{pct(rate)}</span>
        <span style={{ color: "#d7e4ff", fontWeight: 600 }}>{usd(amount)}</span>
      </div>
    </article>
  );
}

function MetricTiny({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return (
    <div style={{ border: "1px solid var(--line)", borderRadius: 10, padding: 10, background: "rgba(9, 16, 33, 0.82)" }}>
      <div style={{ fontSize: 12, color: "var(--muted)", marginBottom: 4 }}>{label}</div>
      <div style={{ fontWeight: 700, color: ok ? "#38f9a5" : "#ff8f8f" }}>{value}</div>
    </div>
  );
}

function StatusPill({
  label,
  value,
  ok,
  icon,
}: {
  label: string;
  value: string;
  ok: boolean;
  icon: "db" | "redis" | "executor" | "bybit" | "recon";
}) {
  const tone = ok
    ? { color: "#38f9a5", borderColor: "rgba(56,249,165,0.24)", iconBg: "rgba(9, 35, 28, 0.72)" }
    : { color: "#ff8f8f", borderColor: "rgba(255,100,120,0.22)", iconBg: "rgba(45, 17, 24, 0.72)" };
  return (
    <div style={{ ...statusPillStyle, borderColor: tone.borderColor }}>
      <span style={{ ...statusPillIconStyle, color: tone.color, borderColor: tone.borderColor, background: tone.iconBg }}>
        {renderStatusIcon(icon)}
      </span>
      <div style={{ minWidth: 0 }}>
        <div style={statusPillLabelStyle}>{label}</div>
        <div style={{ ...statusPillValueStyle, color: tone.color }}>{value}</div>
      </div>
    </div>
  );
}

function renderStatusIcon(icon: "db" | "redis" | "executor" | "bybit" | "recon") {
  const common = { width: 14, height: 14, viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: 1.5, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };
  switch (icon) {
    case "db":
      return (
        <svg {...common}>
          <ellipse cx="8" cy="3.5" rx="4.5" ry="2.2" />
          <path d="M3.5 3.5v4.5c0 1.2 2 2.2 4.5 2.2s4.5-1 4.5-2.2V3.5" />
          <path d="M3.5 8v4.5c0 1.2 2 2.2 4.5 2.2s4.5-1 4.5-2.2V8" />
        </svg>
      );
    case "redis":
      return (
        <svg {...common}>
          <path d="M4 5.2 8 3l4 2.2L8 7.4 4 5.2Z" />
          <path d="M4 8 8 5.8 12 8 8 10.2 4 8Z" />
          <path d="M4 10.8 8 8.6l4 2.2L8 13 4 10.8Z" />
        </svg>
      );
    case "executor":
      return (
        <svg {...common}>
          <circle cx="8" cy="8" r="5.5" />
          <path d="m7 5 4 3-4 3V5Z" />
        </svg>
      );
    case "bybit":
      return (
        <svg {...common}>
          <path d="M5 3.5h2.2a2 2 0 0 1 0 4H5V3.5Z" />
          <path d="M5 7.5h2.7a2 2 0 1 1 0 4H5v-4Z" />
          <path d="M10.5 4.2v7.6" />
        </svg>
      );
    case "recon":
      return (
        <svg {...common}>
          <path d="M8 2.5 12.5 5v6L8 13.5 3.5 11V5L8 2.5Z" />
          <path d="M6.3 8 7.4 9.1 9.9 6.6" />
        </svg>
      );
  }
}

function TokenDecisionBoardCard({ item }: { item: TokenDecisionCardData }) {
  const stateStyle =
    item.stateTone === "positive"
      ? statusTone(true)
      : item.stateTone === "negative"
        ? statusTone(false)
        : { borderColor: "rgba(120,148,205,0.35)", color: "#a7bddf", background: "rgba(33,54,96,0.18)" };
  const trendPositive = item.trendScore >= 0;
  return (
    <article style={tokenCardStyle}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10, marginBottom: 10 }}>
        <strong style={{ fontSize: 15 }}>{item.symbol}</strong>
        <span style={{ ...miniBadgeStyle, ...stateStyle }}>{item.stateLabel}</span>
      </div>
      <div style={tokenHeadlineStyle}>
        <span style={{ color: trendPositive ? "#dff4ff" : "#ffd9dd" }}>
          Trend {item.trendScore >= 0 ? "+" : ""}
          {num(item.trendScore, 3)}
        </span>
        <span style={{ color: "#8fb4e4", fontSize: 13, fontWeight: 600 }}>
          {item.consensusCount}/{item.exchangesAvailable} consensus
        </span>
      </div>
      <div style={tokenDetailGridStyle}>
        <div style={tokenMetricStyle}>
          <div style={tokenMetricLabelStyle}>Bybit</div>
          <div style={tokenMetricValueStyle}>{item.bybitRegime}</div>
        </div>
        <div style={tokenMetricStyle}>
          <div style={tokenMetricLabelStyle}>Recent</div>
          <div style={tokenChipRowStyle}>
            <span style={tokenChipStyle}>B {item.recentBuy}</span>
            <span style={tokenChipStyle}>H {item.recentHold}</span>
            <span style={tokenChipStyle}>S {item.recentSell}</span>
          </div>
        </div>
      </div>
      {item.priorityRank > 0 && <div style={tokenFooterStyle}>Block: {item.mainBlock}</div>}
    </article>
  );
}

function ServiceFlowGraph({
  services,
  executionMode,
  executorVisualState,
}: {
  services: DashboardPayload["services"];
  executionMode: "paper" | "testnet" | "mainnet";
  executorVisualState: "running" | "paused" | "down";
}) {
  const serviceMap = new Map(services.map((svc) => [svc.name, svc]));
  const nodes =
    executionMode === "testnet"
      ? ([
          { name: "bybit", x: 130, y: 250 },
          { name: "market-data", x: 360, y: 250 },
          { name: "strategy", x: 585, y: 250 },
          { name: "executor", x: 790, y: 250 },
          { name: "api", x: 1092, y: 86 },
          { name: "monitor", x: 1092, y: 194 },
          { name: "analytics", x: 1092, y: 302 },
          { name: "backtest", x: 1092, y: 410 },
        ] as const)
      : ([
          { name: "bybit", x: 110, y: 140 },
          { name: "binance", x: 110, y: 280 },
          { name: "okx", x: 110, y: 420 },
          { name: "market-data", x: 360, y: 280 },
          { name: "strategy", x: 610, y: 280 },
          { name: "executor", x: 820, y: 280 },
          { name: "api", x: 1080, y: 100 },
          { name: "monitor", x: 1080, y: 190 },
          { name: "analytics", x: 1080, y: 280 },
          { name: "backtest", x: 1080, y: 370 },
        ] as const);

  const links =
    executionMode === "testnet"
      ? ([
          ["bybit", "market-data", 0],
          ["market-data", "strategy", 0],
          ["strategy", "executor", 0],
          ["executor", "api", -22],
          ["executor", "monitor", -8],
          ["executor", "analytics", 8],
          ["executor", "backtest", 22],
        ] as const)
      : ([
          ["bybit", "market-data", -10],
          ["binance", "market-data", 0],
          ["okx", "market-data", 10],
          ["market-data", "strategy", 0],
          ["strategy", "executor", 0],
          ["executor", "api", -18],
          ["executor", "monitor", -6],
          ["executor", "analytics", 6],
          ["executor", "backtest", 18],
        ] as const);

  const nodeByName = new Map<string, (typeof nodes)[number]>(
    nodes.map((n) => [n.name, n] as [string, (typeof nodes)[number]]),
  );
  const nodeOk = (name: string) => serviceMap.get(name)?.ok ?? false;
  const nodeColor = (name: string) => serviceColor(name, nodeOk(name));
  const nodeLatency = (name: string) => serviceMap.get(name)?.latency_ms ?? 0;
  const executorStateColor =
    executorVisualState === "running" ? "#38f9a5" : executorVisualState === "paused" ? "#ffd978" : "#ff8f8f";

  return (
    <div style={serviceFlowCanvasStyle}>
      <svg viewBox={executionMode === "testnet" ? "0 0 1200 500" : "0 0 1180 560"} style={{ width: "100%", height: "auto", display: "block" }}>
        <defs>
          <radialGradient id="viper-hub-glow" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stopColor="rgba(0,229,168,0.35)" />
            <stop offset="100%" stopColor="rgba(0,229,168,0)" />
          </radialGradient>
        </defs>

        <rect x={0} y={0} width={executionMode === "testnet" ? 1200 : 1180} height={executionMode === "testnet" ? 500 : 560} fill="rgba(4,10,20,0.35)" />
        <circle cx={executionMode === "testnet" ? 790 : 820} cy={executionMode === "testnet" ? 250 : 280} r={executionMode === "testnet" ? 130 : 170} fill="url(#viper-hub-glow)" />

        {links.map(([from, to, curve]) => {
          const a = nodeByName.get(from);
          const b = nodeByName.get(to);
          if (!a || !b) return null;
          const path = `M ${a.x} ${a.y} C ${a.x + 140} ${a.y + curve}, ${b.x - 140} ${b.y + curve}, ${b.x} ${b.y}`;
          const active = nodeOk(from) && nodeOk(to);
          return (
            <g key={`${from}-${to}`}>
              <path
                d={path}
                fill="none"
                stroke={active ? "rgba(95,145,255,0.55)" : "rgba(255,100,120,0.25)"}
                strokeWidth={active ? 1.8 : 1.2}
                className={active ? "service-flow-link" : undefined}
              />
              {active && (
                <circle r={2.6} fill="#9fc4ff" className="service-flow-dot">
                  <animateMotion dur="3.8s" repeatCount="indefinite" path={path} />
                </circle>
              )}
            </g>
          );
        })}

        {nodes.map((node) => {
          const ok = nodeOk(node.name);
          const color = nodeColor(node.name);
          const latencyLabel = `${nodeLatency(node.name)}ms`;
          const executorSubLabel =
            node.name === "executor"
              ? `${executorVisualState} · ${executionMode}`
              : "";
          const isTestnetRightColumn =
            executionMode === "testnet" &&
            (node.name === "api" ||
              node.name === "monitor" ||
              node.name === "analytics" ||
              node.name === "backtest");
          const labelOffsetY = isTestnetRightColumn ? -28 : -32;
          const latencyOffsetY = isTestnetRightColumn ? 34 : 40;
          const detailOffsetY = isTestnetRightColumn ? 50 : 56;
          return (
            <g key={node.name}>
              <circle cx={node.x} cy={node.y} r={ok ? 16 : 14} fill="rgba(7,16,30,0.95)" stroke={color} strokeWidth={1.6} />
              {ok && <circle cx={node.x} cy={node.y} r={26} fill="none" stroke={color} strokeWidth={1} opacity={0.35} className="service-flow-ring" />}
              <circle cx={node.x} cy={node.y} r={5} fill={color} opacity={0.92} />
              <text x={node.x} y={node.y + labelOffsetY} fill="#cadcff" fontSize="12" fontWeight="700" textAnchor="middle">
                {node.name}
              </text>
              <text x={node.x} y={node.y + latencyOffsetY} fill="#8da7d7" fontSize="11" fontWeight="600" textAnchor="middle">
                {latencyLabel}
              </text>
              {executorSubLabel && (
                <text x={node.x} y={node.y + detailOffsetY} fill="#8da7d7" fontSize="11" textAnchor="middle">
                  {executorSubLabel}
                </text>
              )}
              {!ok && node.name !== "executor" && (
                <text x={node.x} y={node.y + detailOffsetY} fill="#ff9aa8" fontSize="11" textAnchor="middle">
                  down
                </text>
              )}
              {node.name === "executor" && (
                <circle cx={node.x} cy={node.y} r={31} fill="none" stroke={executorStateColor} strokeWidth={1.2} opacity={0.32} />
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function Th({ children }: { children: ReactNode }) {
  return (
    <th style={{ textAlign: "left", borderBottom: "1px solid var(--line)", padding: "10px 10px", color: "#95b1df", fontWeight: 600, fontSize: 12 }}>
      {children}
    </th>
  );
}

function Td({ children, style, colSpan }: { children: ReactNode; style?: CSSProperties; colSpan?: number }) {
  return (
    <td
      colSpan={colSpan}
      style={{ borderBottom: "1px solid rgba(95,137,203,0.15)", padding: "9px 10px", verticalAlign: "top", fontSize: 13, ...style }}
    >
      {children}
    </td>
  );
}

const shellStyle: CSSProperties = {
  padding: "24px clamp(14px, 3vw, 28px) 32px",
  maxWidth: 1460,
  margin: "0 auto",
  display: "grid",
  gap: 16,
};

const heroStyle: CSSProperties = {
  border: "1px solid rgba(95, 137, 203, 0.35)",
  borderRadius: 18,
  padding: "18px clamp(14px, 2vw, 24px)",
  background:
    "linear-gradient(130deg, rgba(7,18,37,0.95) 0%, rgba(9,29,60,0.85) 42%, rgba(11,44,62,0.65) 100%)",
  boxShadow: "0 18px 45px rgba(2, 6, 14, 0.5)",
};

const heroGridStyle: CSSProperties = {
  marginTop: 16,
  display: "grid",
  gap: 12,
  gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))",
};

const heroWalletSectionStyle: CSSProperties = {
  marginTop: 16,
  paddingTop: 16,
  borderTop: "1px solid rgba(95, 137, 203, 0.18)",
};

const heroOpsSectionStyle: CSSProperties = {
  marginTop: 16,
  paddingTop: 16,
  borderTop: "1px solid rgba(95, 137, 203, 0.18)",
};

const walletTopStripStyle: CSSProperties = {
  display: "grid",
  gap: 10,
  gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))",
  marginBottom: 14,
};

const walletRateCardStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  border: "1px solid rgba(95,137,203,0.22)",
  borderRadius: 12,
  padding: "10px 12px",
  background: "linear-gradient(180deg, rgba(8,18,36,0.86), rgba(6,14,28,0.92))",
};

const splitTwoStyle: CSSProperties = {
  display: "grid",
  gap: 14,
  gridTemplateColumns: "minmax(0, 1fr) minmax(180px, 220px)",
  alignItems: "start",
};

const stackCardsStyle: CSSProperties = {
  display: "grid",
  gap: 14,
  gridTemplateColumns: "1fr",
};

const panelBoxStyle: CSSProperties = {
  border: "1px solid var(--line)",
  borderRadius: 14,
  padding: 14,
  background: "var(--panel)",
  backdropFilter: "blur(6px)",
};

const flowWrapStyle: CSSProperties = {
  display: "flex",
  gap: 10,
  flexWrap: "wrap",
  alignItems: "center",
  marginBottom: 14,
};

const flowNodeStyle: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 8,
  border: "1px solid",
  borderRadius: 999,
  padding: "7px 10px",
  background: "rgba(6, 15, 30, 0.9)",
};

const serviceFlowCanvasStyle: CSSProperties = {
  border: "1px solid rgba(95,137,203,0.2)",
  borderRadius: 12,
  overflow: "hidden",
  minHeight: 420,
  background:
    "radial-gradient(circle at 50% 46%, rgba(0,229,168,0.1) 0%, rgba(0,229,168,0) 38%), linear-gradient(180deg, rgba(6,14,27,0.92), rgba(4,10,20,0.92))",
};

const tableStyle: CSSProperties = {
  width: "100%",
  borderCollapse: "collapse",
  border: "1px solid var(--line)",
  borderRadius: 12,
  overflow: "hidden",
  background: "var(--panel-strong)",
};

const h2Style: CSSProperties = {
  margin: 0,
  fontSize: 18,
};

const h3Style: CSSProperties = {
  margin: 0,
  fontSize: 15,
  color: "#9fb9e5",
};

const mutedStyle: CSSProperties = {
  color: "var(--muted)",
  marginTop: 6,
};

const labelStyle: CSSProperties = {
  marginBottom: 6,
  fontSize: 12,
  color: "#9cb8e6",
};

const inputStyle: CSSProperties = {
  width: "100%",
  height: 36,
  border: "1px solid var(--line)",
  borderRadius: 8,
  padding: "0 10px",
  boxSizing: "border-box",
  background: "rgba(8, 18, 36, 0.9)",
  color: "#e5eeff",
};

const btnBase: CSSProperties = {
  height: 36,
  border: "1px solid",
  borderRadius: 8,
  padding: "0 14px",
  cursor: "pointer",
  fontWeight: 600,
};

const btnStylePrimary: CSSProperties = {
  ...btnBase,
  borderColor: "rgba(0,229,168,0.8)",
  background: "rgba(0, 139, 103, 0.24)",
  color: "#4afac0",
};

const btnStyleSecondary: CSSProperties = {
  ...btnBase,
  borderColor: "rgba(17,196,255,0.8)",
  background: "rgba(17, 102, 138, 0.24)",
  color: "#90dcff",
};

const miniBadgeStyle: CSSProperties = {
  border: "1px solid",
  borderRadius: 999,
  padding: "3px 8px",
  fontSize: 11,
  fontWeight: 700,
};

const miniGridStyle: CSSProperties = {
  display: "grid",
  gap: 10,
  gridTemplateColumns: "repeat(auto-fit, minmax(170px, 1fr))",
};

const statusPillGridStyle: CSSProperties = {
  display: "grid",
  gap: 10,
};

const statusPillStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  border: "1px solid",
  borderRadius: 12,
  padding: "10px 12px",
};

const statusPillIconStyle: CSSProperties = {
  width: 24,
  height: 24,
  borderRadius: 999,
  border: "1px solid",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: 11,
  fontWeight: 800,
  flexShrink: 0,
};

const statusPillLabelStyle: CSSProperties = {
  fontSize: 11,
  color: "#91abd6",
  textTransform: "uppercase",
  letterSpacing: 0.4,
  marginBottom: 2,
};

const statusPillValueStyle: CSSProperties = {
  fontSize: 14,
  fontWeight: 700,
};

const tokenGridStyle: CSSProperties = {
  display: "grid",
  gap: 10,
  gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
};

const tokenCardStyle: CSSProperties = {
  border: "1px solid var(--line)",
  borderRadius: 12,
  padding: 12,
  background: "linear-gradient(180deg, rgba(8,18,36,0.86), rgba(6,14,28,0.94))",
};

const tokenMetricStyle: CSSProperties = {
  border: "1px solid rgba(95,137,203,0.18)",
  borderRadius: 8,
  padding: "8px 10px",
  background: "rgba(8, 19, 38, 0.8)",
};

const marketSignalGridStyle: CSSProperties = {
  display: "grid",
  gap: 8,
  gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
};

const tokenHeadlineStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 10,
  marginBottom: 10,
  fontSize: 15,
  fontWeight: 700,
};

const tokenDetailGridStyle: CSSProperties = {
  display: "grid",
  gap: 8,
  gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
};

const tokenChipRowStyle: CSSProperties = {
  display: "flex",
  gap: 6,
  flexWrap: "wrap",
};

const tokenChipStyle: CSSProperties = {
  border: "1px solid rgba(95,137,203,0.2)",
  borderRadius: 999,
  padding: "2px 7px",
  fontSize: 11,
  color: "#cfe0ff",
  background: "rgba(13, 24, 46, 0.88)",
};

const tokenMetricLabelStyle: CSSProperties = {
  fontSize: 11,
  color: "#8eaad6",
  textTransform: "uppercase",
  letterSpacing: 0.35,
  marginBottom: 4,
};

const tokenMetricValueStyle: CSSProperties = {
  fontSize: 13,
  color: "#dbe8ff",
  fontWeight: 600,
  lineHeight: 1.35,
};

const tokenFooterStyle: CSSProperties = {
  marginTop: 10,
  paddingTop: 10,
  borderTop: "1px solid rgba(95, 137, 203, 0.16)",
  fontSize: 12,
  color: "#7f99c8",
};

const exchangeRankGridStyle: CSSProperties = {
  display: "grid",
  gap: 10,
  gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
};

const exchangeRankCardStyle: CSSProperties = {
  border: "1px solid rgba(95,137,203,0.24)",
  borderRadius: 10,
  padding: "12px 12px",
  background: "linear-gradient(180deg, rgba(7,14,28,0.88), rgba(6,12,24,0.92))",
};

const exchangeMetricsRowStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 16,
  margin: "10px 0 12px",
};

const exchangeMetricLabelStyle: CSSProperties = {
  fontSize: 11,
  color: "#8eaad6",
  textTransform: "uppercase",
  letterSpacing: 0.35,
  marginBottom: 3,
};

const exchangeMetricValueStyle: CSSProperties = {
  fontSize: 20,
  fontWeight: 700,
  lineHeight: 1.05,
};

const exchangeBarTrackStyle: CSSProperties = {
  width: "100%",
  height: 8,
  borderRadius: 999,
  background: "rgba(35, 56, 96, 0.32)",
  overflow: "hidden",
};

const exchangeBarFillStyle: CSSProperties = {
  height: "100%",
  borderRadius: 999,
};

const tradeStatsGridStyle: CSSProperties = {
  display: "grid",
  gap: 10,
  gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
  marginBottom: 12,
};

const sectionDividerStyle: CSSProperties = {
  margin: "10px 0 14px",
  borderTop: "1px solid rgba(95, 137, 203, 0.18)",
};

const tradeStatCardStyle: CSSProperties = {
  border: "1px solid rgba(95,137,203,0.28)",
  borderRadius: 8,
  padding: "8px 10px",
  background: "rgba(8, 16, 32, 0.8)",
};

const tradeStatLabelStyle: CSSProperties = {
  fontSize: 12,
  color: "#96afda",
  marginBottom: 4,
};

const tradeStatValueStyle: CSSProperties = {
  fontSize: 22,
  fontWeight: 700,
  color: "#dbe8ff",
  lineHeight: 1.1,
};

const tradeListStyle: CSSProperties = {
  display: "grid",
  gap: 8,
};

const tradeRowStyle: CSSProperties = {
  display: "grid",
  gap: 8,
  alignItems: "center",
  gridTemplateColumns: "minmax(150px, 1.5fr) minmax(110px, 1fr) minmax(90px, 0.8fr) minmax(84px, 0.8fr)",
  border: "1px solid rgba(95,137,203,0.24)",
  borderRadius: 8,
  padding: "9px 10px",
  background: "linear-gradient(180deg, rgba(7,14,28,0.88), rgba(6,12,24,0.92))",
};

const positionListStyle: CSSProperties = {
  display: "grid",
  gap: 8,
};

const positionRowStyle: CSSProperties = {
  display: "grid",
  gap: 8,
  alignItems: "center",
  gridTemplateColumns:
    "minmax(140px, 1.1fr) minmax(88px, 0.72fr) minmax(88px, 0.72fr) minmax(112px, 0.92fr) minmax(138px, 1fr) minmax(132px, 0.92fr)",
  border: "1px solid rgba(95,137,203,0.24)",
  borderRadius: 8,
  padding: "9px 10px",
  background: "linear-gradient(180deg, rgba(7,14,28,0.88), rgba(6,12,24,0.92))",
  overflowX: "auto",
};

const positionLabelStyle: CSSProperties = {
  fontSize: 11,
  color: "#92a9d2",
  marginBottom: 2,
};
