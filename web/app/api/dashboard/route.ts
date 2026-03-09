import { NextResponse } from "next/server";

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

function resolveBybitRestUrl(): string {
  const explicitUrl = process.env.BYBIT_REST_URL || process.env.NEXT_PUBLIC_BYBIT_REST_URL;
  if (explicitUrl && explicitUrl.trim()) return explicitUrl.trim();

  const envRaw = process.env.BYBIT_ENV || process.env.NEXT_PUBLIC_BYBIT_ENV || "testnet";
  const env = envRaw.trim().toLowerCase();
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

    return {
      name: "",
      ok: response.ok,
      status: response.status,
      latency_ms: Date.now() - startedAt,
      url,
      error: response.ok ? undefined : response.statusText,
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

async function fetchServices(baseUrl: string): Promise<ServiceHealth[]> {
  const parsed = new URL(baseUrl);
  const host = parsed.hostname;
  const protocol = parsed.protocol;
  const bybitRestUrl = resolveBybitRestUrl().replace(/\/+$/, "");

  const targets: Array<{ name: string; urls: string[] }> = [
    { name: "api", urls: [`${protocol}//${host}:8080/api/v1/health`, `${protocol}//${host}:8080/health`] },
    { name: "market-data", urls: [`${protocol}//${host}:8081/health`] },
    { name: "strategy", urls: [`${protocol}//${host}:8082/health`] },
    { name: "executor", urls: [`${protocol}//${host}:8083/health`] },
    { name: "monitor", urls: [`${protocol}//${host}:8084/health`] },
    { name: "backtest", urls: [`${protocol}//${host}:8085/health`] },
    { name: "bybit", urls: [`${bybitRestUrl}/v5/market/time`] },
  ];

  const checks = await Promise.all(
    targets.map(async (target) => {
      for (const candidate of target.urls) {
        const result = await checkServiceUrl(candidate);
        if (result.ok) {
          return { ...result, name: target.name };
        }
      }
      const last = await checkServiceUrl(target.urls[target.urls.length - 1]);
      return { ...last, name: target.name };
    }),
  );

  const bybitPrivate = await fetchJson(baseUrl, "/external/bybit-private-health");
  const bybitPrivateService: ServiceHealth = bybitPrivate.ok
    ? ({
        name: "bybit-private",
        ...(bybitPrivate.data as ServiceHealth),
      } as ServiceHealth)
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

export async function GET() {
  const baseUrls = uniqueBaseUrls(DEFAULT_BASE_URLS);
  const errors: BackendError[] = [];

  for (const baseUrl of baseUrls) {
    const status = await fetchJson(baseUrl, "/status");
    if (!status.ok) {
      errors.push({ baseUrl, message: `status failed: ${status.error}` });
      continue;
    }

    const [performance, positions, trades, events, riskKpis, controlState, services] = await Promise.all([
      fetchJson(baseUrl, "/performance"),
      fetchJson(baseUrl, "/positions"),
      fetchJson(baseUrl, "/trades?limit=20"),
      fetchJson(baseUrl, "/events?limit=40"),
      fetchJson(baseUrl, "/risk/kpis"),
      fetchJson(baseUrl, "/control/state"),
      fetchServices(baseUrl),
    ]);

    const partialErrors: string[] = [];
    if (!performance.ok) partialErrors.push(`performance failed: ${performance.error}`);
    if (!positions.ok) partialErrors.push(`positions failed: ${positions.error}`);
    if (!trades.ok) partialErrors.push(`trades failed: ${trades.error}`);
    if (!events.ok) partialErrors.push(`events failed: ${events.error}`);
    if (!riskKpis.ok) partialErrors.push(`risk_kpis failed: ${riskKpis.error}`);
    if (!controlState.ok) partialErrors.push(`control_state failed: ${controlState.error}`);

    return NextResponse.json(
      {
        source: { baseUrl, fetchedAt: new Date().toISOString() },
        status: status.data,
        performance: performance.ok ? performance.data : { error: "unavailable" },
        positions: positions.ok ? positions.data : { items: [] },
        trades: trades.ok ? trades.data : { items: [] },
        events: events.ok ? events.data : { items: [] },
        risk_kpis: riskKpis.ok
          ? riskKpis.data
          : {
              rejected_orders_24h: 0,
              open_exposure_usdt: 0,
              realized_pnl_24h: 0,
              critical_events_24h: 0,
              circuit_breaker_triggers_24h: 0,
            },
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
