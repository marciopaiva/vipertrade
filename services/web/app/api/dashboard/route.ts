import { NextResponse } from "next/server";

export const dynamic = "force-dynamic";
export const revalidate = 0;
export const fetchCache = "force-no-store";

type BackendError = {
  baseUrl: string;
  message: string;
};

type FetchJsonResult = {
  ok: boolean;
  status: number;
  data?: unknown;
  error?: string;
};

type ServiceHealth = {
  name: string;
  ok: boolean;
  status: number;
  latency_ms: number;
  url: string;
  error?: string;
  invalid_market_signals_dropped?: number;
  last_invalid_market_signal_drop?: {
    symbol?: string;
    stage?: string;
    reason?: string;
    timestamp?: string;
  };
};

type AnalyticsScores = {
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

type WalletSummary = {
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

type DailyTradesSummary = {
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

const DEFAULT_BASE_URLS = [
  process.env.BACKEND_API_URL,
  "http://host.containers.internal:8080/api/v1",
  "http://host.docker.internal:8080/api/v1",
  "http://api:8080/api/v1",
  "http://vipertrade-api:8080/api/v1",
  process.env.NEXT_PUBLIC_API_URL,
  "http://localhost:8080/api/v1",
  "http://127.0.0.1:8080/api/v1",
].filter(Boolean) as string[];

function uniqueBaseUrls(baseUrls: string[]): string[] {
  return Array.from(new Set(baseUrls.map((v) => v.replace(/\/+$/, ""))));
}

function resolveBybitRestUrl(tradingMode?: string): string {
  const explicitUrl = process.env.BYBIT_REST_URL || process.env.NEXT_PUBLIC_BYBIT_REST_URL;
  if (explicitUrl && explicitUrl.trim()) return explicitUrl.trim();

  const mode = (tradingMode || process.env.TRADING_MODE || "").trim().toLowerCase();
  const env =
    mode === "testnet"
      ? "testnet"
      : mode === "paper" || mode === "mainnet" || mode === "live"
        ? "mainnet"
        : (process.env.BYBIT_ENV || process.env.NEXT_PUBLIC_BYBIT_ENV || "testnet").trim().toLowerCase();
  return env === "mainnet" ? "https://api.bybit.com" : "https://api-testnet.bybit.com";
}

async function fetchJson(baseUrl: string, path: string): Promise<FetchJsonResult> {
  const url = `${baseUrl}${path}`;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 5000);

  try {
    const response = await fetch(url, {
      method: "GET",
      headers: { accept: "application/json" },
      cache: "no-store",
      signal: controller.signal,
    });

    const rawBody = await response.text();
    const parsed = rawBody ? JSON.parse(rawBody) : null;

    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        error: `http=${response.status} body=${rawBody || "<empty>"}`,
      };
    }

    return { ok: true, status: response.status, data: parsed };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { ok: false, status: 0, error: message };
  } finally {
    clearTimeout(timeout);
  }
}

async function checkServiceUrl(url: string): Promise<ServiceHealth> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 2500);
  const startedAt = Date.now();

  try {
    const response = await fetch(url, {
      method: "GET",
      headers: { accept: "application/json" },
      cache: "no-store",
      signal: controller.signal,
    });

    const rawBody = await response.text();
    let invalidDropped: number | undefined;
    let lastInvalidDrop:
      | {
          symbol?: string;
          stage?: string;
          reason?: string;
          timestamp?: string;
        }
      | undefined;
    if (rawBody) {
      try {
        const parsed = JSON.parse(rawBody) as {
          invalid_market_signals_dropped?: unknown;
          last_invalid_market_signal_drop?: {
            symbol?: string;
            stage?: string;
            reason?: string;
            timestamp?: string;
          };
        };
        if (typeof parsed.invalid_market_signals_dropped === "number") {
          invalidDropped = parsed.invalid_market_signals_dropped;
        }
        if (parsed.last_invalid_market_signal_drop && typeof parsed.last_invalid_market_signal_drop === "object") {
          lastInvalidDrop = parsed.last_invalid_market_signal_drop;
        }
      } catch {
        // plain-text health endpoints are still valid
      }
    }

    return {
      name: "",
      ok: response.ok,
      status: response.status,
      latency_ms: Date.now() - startedAt,
      url,
      error: response.ok ? undefined : response.statusText,
      invalid_market_signals_dropped: invalidDropped,
      last_invalid_market_signal_drop: lastInvalidDrop,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      name: "",
      ok: false,
      status: 0,
      latency_ms: Date.now() - startedAt,
      url,
      error: message,
    };
  } finally {
    clearTimeout(timeout);
  }
}

async function fetchServices(baseUrl: string, tradingMode?: string): Promise<ServiceHealth[]> {
  const parsed = new URL(baseUrl);
  const host = parsed.hostname;
  const protocol = parsed.protocol;
  const bybitRestUrl = resolveBybitRestUrl(tradingMode).replace(/\/+$/, "");
  const binanceRestUrl = (process.env.BINANCE_REST_URL || "https://fapi.binance.com").replace(/\/+$/, "");
  const okxRestUrl = (process.env.OKX_REST_URL || "https://www.okx.com").replace(/\/+$/, "");

  const targets: Array<{ name: string; urls: string[] }> = [
    { name: "api", urls: [`${protocol}//${host}:8080/api/v1/health`, `${protocol}//${host}:8080/health`] },
    { name: "market-data", urls: [`${protocol}//${host}:8081/health`] },
    { name: "strategy", urls: [`${protocol}//${host}:8082/health`] },
    { name: "executor", urls: [`${protocol}//${host}:8083/health`] },
    { name: "monitor", urls: [`${protocol}//${host}:8084/health`] },
    { name: "analytics", urls: [`${protocol}//${host}:8086/health`] },
    { name: "backtest", urls: [`${protocol}//${host}:8085/health`] },
    { name: "ai-analyst", urls: [`${protocol}//${host}:8087/health`] },
    { name: "bybit", urls: [`${bybitRestUrl}/v5/market/time`] },
    { name: "binance", urls: [`${binanceRestUrl}/fapi/v1/time`] },
    { name: "okx", urls: [`${okxRestUrl}/api/v5/public/time`] },
  ];

  const checks = await Promise.all(
    targets.map(async (target) => {
      for (const candidate of target.urls) {
        const result = await checkServiceUrl(candidate);
        if (result.ok) {
          return { ...result, name: target.name };
        }
      }
      const last = await checkServiceUrl(target.urls[target.urls.length - 1]!);
      return { ...last, name: target.name };
    }),
  );

  const bybitPrivate = await fetchJson(baseUrl, "/external/bybit-private-health");
  const bybitPrivateService: ServiceHealth = bybitPrivate.ok
    ? {
        ...(bybitPrivate.data as ServiceHealth),
        name: "bybit-private",
      }
    : {
        name: "bybit-private",
        ok: false,
        status: 0,
        latency_ms: 0,
        url: `${bybitRestUrl}/v5/account/wallet-balance?accountType=UNIFIED`,
        error: bybitPrivate.error || "backend check unavailable",
      };

  return [...checks, bybitPrivateService];
}

async function fetchMarketSignals(baseUrl: string): Promise<FetchJsonResult> {
  const parsed = new URL(baseUrl);
  const host = parsed.hostname;
  const protocol = parsed.protocol;
  return fetchJson(`${protocol}//${host}:8081`, "/latest-signals");
}

async function fetchAnalyticsScores(baseUrl: string): Promise<FetchJsonResult> {
  const parsed = new URL(baseUrl);
  const host = parsed.hostname;
  const protocol = parsed.protocol;
  return fetchJson(`${protocol}//${host}:8086`, "/scores");
}

async function fetchAiAnalyst(baseUrl: string): Promise<FetchJsonResult> {
  const parsed = new URL(baseUrl);
  const host = parsed.hostname;
  const protocol = parsed.protocol;
  const hours = Number(process.env.AI_ANALYST_LOOKBACK_HOURS || "24");
  const safeHours = Number.isFinite(hours) && hours > 0 ? Math.min(hours, 24 * 14) : 24;
  return fetchJson(`${protocol}//${host}:8087`, `/analyze/recent?hours=${safeHours}`);
}

export async function GET() {
  const baseUrls = uniqueBaseUrls(DEFAULT_BASE_URLS);
  const errors: BackendError[] = [];

  for (const baseUrl of baseUrls) {
    const status = await fetchJson(baseUrl, "/status");
    if (!status.ok) {
      errors.push({ baseUrl, message: `status failed: ${status.error}` });
      continue;
    }

    const servicesPromise = fetchServices(
      baseUrl,
      (status.data as { trading_mode?: string } | undefined)?.trading_mode,
    );
    const [performance, positions, trades, dailyTradesSummary, events, marketSignals, analyticsScores, aiAnalyst, riskKpis, controlState, wallet, services] = await Promise.all([
      fetchJson(baseUrl, "/performance"),
      fetchJson(baseUrl, "/positions"),
      fetchJson(baseUrl, "/trades?limit=100"),
      fetchJson(baseUrl, "/trades/today-summary"),
      fetchJson(baseUrl, "/events?limit=40"),
      fetchMarketSignals(baseUrl),
      fetchAnalyticsScores(baseUrl),
      fetchAiAnalyst(baseUrl),
      fetchJson(baseUrl, "/risk/kpis"),
      fetchJson(baseUrl, "/control/state"),
      fetchJson(baseUrl, "/external/bybit-wallet"),
      servicesPromise,
    ]);

    const partialErrors: string[] = [];
    if (!performance.ok) partialErrors.push(`performance failed: ${performance.error}`);
    if (!positions.ok) partialErrors.push(`positions failed: ${positions.error}`);
    if (!trades.ok) partialErrors.push(`trades failed: ${trades.error}`);
    if (!dailyTradesSummary.ok) partialErrors.push(`daily_trades_summary failed: ${dailyTradesSummary.error}`);
    if (!events.ok) partialErrors.push(`events failed: ${events.error}`);
    if (!marketSignals.ok) partialErrors.push(`market_signals failed: ${marketSignals.error}`);
    if (!analyticsScores.ok) partialErrors.push(`analytics_scores failed: ${analyticsScores.error}`);
    if (!aiAnalyst.ok) partialErrors.push(`ai_analyst failed: ${aiAnalyst.error}`);
    if (!riskKpis.ok) partialErrors.push(`risk_kpis failed: ${riskKpis.error}`);
    if (!controlState.ok) partialErrors.push(`control_state failed: ${controlState.error}`);
    if (!wallet.ok) partialErrors.push(`wallet failed: ${wallet.error}`);

    return NextResponse.json(
      {
        source: { baseUrl, fetchedAt: new Date().toISOString() },
        status: status.data,
        performance: performance.ok ? performance.data : { error: "unavailable" },
        positions: positions.ok ? positions.data : { items: [] },
        trades: trades.ok ? trades.data : { items: [] },
        daily_trades_summary: dailyTradesSummary.ok
          ? dailyTradesSummary.data
          : ({
              ok: false,
              source: "unavailable",
              count: 0,
              error: "unavailable",
            } as DailyTradesSummary),
        events: events.ok ? events.data : { items: [] },
        market_signals: marketSignals.ok ? marketSignals.data : { updated_at: undefined, items: {} },
        analytics_scores: analyticsScores.ok ? analyticsScores.data : { updated_at: undefined, exchanges: [], by_symbol: [] } as unknown as AnalyticsScores,
        ai_analyst: aiAnalyst.ok
          ? aiAnalyst.data
          : {
              generated_at: undefined,
              lookback_hours: Number(process.env.AI_ANALYST_LOOKBACK_HOURS || "24"),
              summary: {
                closed_trades: 0,
                total_pnl_usdt: 0,
                avg_pnl_pct: 0,
                avg_duration_s: 0,
                win_rate_pct: 0,
              },
              tupa_snapshot: undefined,
              tupa_evaluation: undefined,
              tupa_error: aiAnalyst.error || "unavailable",
              heuristic_summary: "AI analyst unavailable.",
              llm_summary: null,
            },
        risk_kpis: riskKpis.ok
          ? riskKpis.data
          : {
              rejected_orders_24h: 0,
              open_exposure_usdt: 0,
              realized_pnl_24h: 0,
              critical_events_24h: 0,
              circuit_breaker_triggers_24h: 0,
            },
        wallet: wallet.ok
          ? wallet.data
          : ({
              ok: false,
              status: 0,
              error: "unavailable",
              account_type: process.env.BYBIT_ACCOUNT_TYPE || "UNIFIED",
            } as WalletSummary),
        control_state: controlState.ok
          ? controlState.data
          : {
              operator_auth_mode: "token",
              operator_controls_enabled: false,
              kill_switch: { enabled: false },
              executor: { enabled: false },
              risk_limits: {
                max_daily_loss_pct: 0,
                max_leverage: 0,
                risk_per_trade_pct: 0,
              },
            },
        services,
        partial: partialErrors.length > 0,
        warnings: partialErrors,
      },
      { status: 200 },
    );
  }

  return NextResponse.json(
    {
      error: "backend_unreachable",
      message: "could not fetch dashboard data from backend",
      tried: baseUrls,
      details: errors,
    },
    { status: 502 },
  );
}
