"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties, ReactNode } from "react";

type DashboardPayload = {
  source: { baseUrl: string; fetchedAt: string };
  status: {
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
  positions: { items?: Array<{ symbol: string; side: string; quantity: number; notional_usdt: number }> };
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
      opened_at: string;
      closed_at?: string | null;
    }>;
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

function num(value: number | null | undefined, digits = 2) {
  if (typeof value !== "number" || Number.isNaN(value)) return "-";
  return value.toFixed(digits);
}

type ControlKind = "kill-switch" | "executor" | "risk-limits";

export default function Home() {
  const [data, setData] = useState<DashboardPayload | null>(null);
  const [error, setError] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [lastUpdated, setLastUpdated] = useState<string>("");

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
      const body = (await res.json()) as DashboardPayload & { message?: string };
      if (!res.ok) {
        throw new Error(body?.message || `failed: ${res.status}`);
      }
      setData(body);
      setError("");
      setLastUpdated(new Date().toISOString());

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

  const checklist = useMemo(
    () => [
      { label: "Backend online", ok: !!data?.status?.db_connected },
      { label: "Operator controls enabled", ok: !!data?.control_state?.operator_controls_enabled },
      { label: "Service health checks available", ok: services.length > 0 },
      { label: "Timeline events available", ok: events.length > 0 },
      {
        label: "Risk KPIs available",
        ok: typeof data?.risk_kpis?.critical_events_24h === "number",
      },
    ],
    [data, events.length, services.length],
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
        const body = (await res.json()) as { message?: string; error?: string; source?: string };
        if (!res.ok) {
          throw new Error(body?.message || body?.error || `failed: ${res.status}`);
        }
        setControlMessage(`Command '${kind}' applied via ${body.source || "backend"}`);
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
    <main style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", padding: 20, maxWidth: 1400, margin: "0 auto" }}>
      <h1 style={{ margin: "0 0 8px 0" }}>ViperTrade Dashboard</h1>
      <p style={{ marginTop: 0, color: "#555" }}>
        Realtime snapshot every 5s {lastUpdated ? `- updated ${new Date(lastUpdated).toLocaleTimeString()}` : ""}
      </p>

      {loading && <p>Loading dashboard...</p>}
      {!!error && <p style={{ color: "#b00020" }}>Error: {error}</p>}
      {!!data?.partial && (
        <div style={{ ...panelStyle, borderColor: "#e6b700", background: "#fff9e6", marginBottom: 16 }}>
          <strong>Partial data mode:</strong>
          <ul style={{ marginTop: 8, marginBottom: 0 }}>
            {(data.warnings ?? []).map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </div>
      )}

      {data && (
        <>
          <section style={{ display: "grid", gap: 12, gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", marginBottom: 20 }}>
            <Card label="Risk Status" value={String(data.status?.risk_status || "-")} />
            <Card label="Trading Mode/Profile" value={`${data.status?.trading_mode || "-"} / ${data.status?.trading_profile || "-"}`} />
            <Card label="DB Connected" value={data.status?.db_connected ? "yes" : "no"} />
            <Card label="Kill Switch" value={data.control_state?.kill_switch?.enabled ? "enabled" : "disabled"} />
            <Card label="Executor" value={data.control_state?.executor?.enabled ? "running" : "paused"} />
            <Card label="Critical Recon (15m)" value={String(data.status?.critical_reconciliation_events_15m ?? "-")} />
          </section>

          <section style={{ display: "grid", gap: 12, gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", marginBottom: 20 }}>
            <Card label="Rejected Orders (24h)" value={String(data.risk_kpis?.rejected_orders_24h ?? 0)} />
            <Card label="Open Exposure (USDT)" value={num(data.risk_kpis?.open_exposure_usdt, 6)} />
            <Card label="Realized PnL (24h)" value={num(data.risk_kpis?.realized_pnl_24h, 6)} />
            <Card label="Critical Events (24h)" value={String(data.risk_kpis?.critical_events_24h ?? 0)} />
            <Card label="Circuit Breakers (24h)" value={String(data.risk_kpis?.circuit_breaker_triggers_24h ?? 0)} />
            <Card label="Max Drawdown (30d)" value={num(data.performance?.max_drawdown_30d, 6)} />
          </section>

          <section style={{ marginBottom: 20 }}>
            <h2 style={{ marginBottom: 8 }}>Service Health (Realtime)</h2>
            <table style={tableStyle}>
              <thead>
                <tr>
                  <Th>Service</Th>
                  <Th>Status</Th>
                  <Th>HTTP</Th>
                  <Th>Latency ms</Th>
                  <Th>Endpoint</Th>
                </tr>
              </thead>
              <tbody>
                {services.map((s) => (
                  <tr key={s.name}>
                    <Td>{s.name}</Td>
                    <Td>{s.ok ? "healthy" : "degraded"}</Td>
                    <Td>{String(s.status)}</Td>
                    <Td>{String(s.latency_ms)}</Td>
                    <Td>{s.url}</Td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>

          <section style={{ marginBottom: 20 }}>
            <h2 style={{ marginBottom: 8 }}>Operator Controls</h2>
            <div style={{ ...panelStyle, marginBottom: 12 }}>
              <div style={{ display: "grid", gap: 12, gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))" }}>
                <label>
                  <div style={labelStyle}>Operator Token</div>
                  <input
                    style={inputStyle}
                    type="password"
                    value={operatorToken}
                    onChange={(e) => setOperatorToken(e.target.value)}
                    placeholder="x-operator-token"
                  />
                </label>
                <label>
                  <div style={labelStyle}>Operator ID</div>
                  <input
                    style={inputStyle}
                    value={operatorId}
                    onChange={(e) => setOperatorId(e.target.value)}
                    placeholder="web-operator"
                  />
                </label>
                <label>
                  <div style={labelStyle}>Reason</div>
                  <input
                    style={inputStyle}
                    value={actionReason}
                    onChange={(e) => setActionReason(e.target.value)}
                    placeholder="manual_web_action"
                  />
                </label>
              </div>

              <div style={{ display: "flex", gap: 10, marginTop: 14, flexWrap: "wrap" }}>
                <button
                  style={btnStyle}
                  disabled={!operatorToken || !!controlBusy}
                  onClick={() =>
                    void sendControl("kill-switch", {
                      enabled: !(data.control_state?.kill_switch?.enabled ?? false),
                      reason: actionReason,
                    })
                  }
                >
                  {controlBusy === "kill-switch"
                    ? "Applying..."
                    : data.control_state?.kill_switch?.enabled
                      ? "Disable Kill Switch"
                      : "Enable Kill Switch"}
                </button>

                <button
                  style={btnStyle}
                  disabled={!operatorToken || !!controlBusy}
                  onClick={() =>
                    void sendControl("executor", {
                      enabled: !(data.control_state?.executor?.enabled ?? false),
                      reason: actionReason,
                    })
                  }
                >
                  {controlBusy === "executor"
                    ? "Applying..."
                    : data.control_state?.executor?.enabled
                      ? "Pause Executor"
                      : "Resume Executor"}
                </button>
              </div>

              <div style={{ marginTop: 14 }}>
                <h3 style={{ margin: "0 0 8px 0", fontSize: 16 }}>Risk Limits</h3>
                <div style={{ display: "grid", gap: 12, gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))" }}>
                  <label>
                    <div style={labelStyle}>Max Daily Loss Pct</div>
                    <input style={inputStyle} value={maxDailyLossPct} onChange={(e) => setMaxDailyLossPct(e.target.value)} />
                  </label>
                  <label>
                    <div style={labelStyle}>Max Leverage</div>
                    <input style={inputStyle} value={maxLeverage} onChange={(e) => setMaxLeverage(e.target.value)} />
                  </label>
                  <label>
                    <div style={labelStyle}>Risk Per Trade Pct</div>
                    <input style={inputStyle} value={riskPerTradePct} onChange={(e) => setRiskPerTradePct(e.target.value)} />
                  </label>
                </div>
                <div style={{ marginTop: 10 }}>
                  <button
                    style={btnStyle}
                    disabled={!operatorToken || !!controlBusy}
                    onClick={() =>
                      void sendControl("risk-limits", {
                        max_daily_loss_pct: Number(maxDailyLossPct),
                        max_leverage: Number(maxLeverage),
                        risk_per_trade_pct: Number(riskPerTradePct),
                        reason: actionReason,
                      })
                    }
                  >
                    {controlBusy === "risk-limits" ? "Applying..." : "Apply Risk Limits"}
                  </button>
                </div>
              </div>

              {!!controlMessage && <p style={{ marginTop: 10, color: "#1d4d1d" }}>{controlMessage}</p>}
            </div>
          </section>

          <section style={{ marginBottom: 20 }}>
            <h2 style={{ marginBottom: 8 }}>Performance</h2>
            <table style={tableStyle}>
              <thead>
                <tr>
                  <Th>Window</Th>
                  <Th>Total Trades</Th>
                  <Th>Total PnL</Th>
                  <Th>Win Rate</Th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <Td>24h</Td>
                  <Td>{String(data.performance?.last_24h?.total_trades ?? "-")}</Td>
                  <Td>{num(data.performance?.last_24h?.total_pnl, 6)}</Td>
                  <Td>{num(data.performance?.last_24h?.win_rate, 6)}</Td>
                </tr>
                <tr>
                  <Td>7d</Td>
                  <Td>{String(data.performance?.last_7d?.total_trades ?? "-")}</Td>
                  <Td>{num(data.performance?.last_7d?.total_pnl, 6)}</Td>
                  <Td>{num(data.performance?.last_7d?.win_rate, 6)}</Td>
                </tr>
                <tr>
                  <Td>30d</Td>
                  <Td>{String(data.performance?.last_30d?.total_trades ?? "-")}</Td>
                  <Td>{num(data.performance?.last_30d?.total_pnl, 6)}</Td>
                  <Td>{num(data.performance?.last_30d?.win_rate, 6)}</Td>
                </tr>
              </tbody>
            </table>
          </section>

          <section style={{ marginBottom: 20 }}>
            <h2 style={{ marginBottom: 8 }}>Timeline (events/signals/executions)</h2>
            <table style={tableStyle}>
              <thead>
                <tr>
                  <Th>Timestamp</Th>
                  <Th>Type</Th>
                  <Th>Severity</Th>
                  <Th>Category</Th>
                  <Th>Symbol</Th>
                  <Th>Data</Th>
                </tr>
              </thead>
              <tbody>
                {events.slice(0, 20).map((evt) => (
                  <tr key={evt.event_id}>
                    <Td>{new Date(evt.timestamp).toLocaleString()}</Td>
                    <Td>{evt.event_type}</Td>
                    <Td>{evt.severity}</Td>
                    <Td>{evt.category || "-"}</Td>
                    <Td>{evt.symbol || "-"}</Td>
                    <Td>
                      <code style={{ fontSize: 12 }}>{JSON.stringify(evt.data ?? {}).slice(0, 140)}</code>
                    </Td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>

          <section style={{ marginBottom: 20 }}>
            <h2 style={{ marginBottom: 8 }}>Open Positions</h2>
            {positions.length === 0 ? (
              <p>No open positions.</p>
            ) : (
              <table style={tableStyle}>
                <thead>
                  <tr>
                    <Th>Symbol</Th>
                    <Th>Side</Th>
                    <Th>Quantity</Th>
                    <Th>Notional USDT</Th>
                  </tr>
                </thead>
                <tbody>
                  {positions.map((p) => (
                    <tr key={`${p.symbol}-${p.side}`}>
                      <Td>{p.symbol}</Td>
                      <Td>{p.side}</Td>
                      <Td>{num(p.quantity, 6)}</Td>
                      <Td>{num(p.notional_usdt, 6)}</Td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section style={{ marginBottom: 20 }}>
            <h2 style={{ marginBottom: 8 }}>Recent Trades (limit 20)</h2>
            {trades.length === 0 ? (
              <p>No trades found.</p>
            ) : (
              <table style={tableStyle}>
                <thead>
                  <tr>
                    <Th>Opened</Th>
                    <Th>Symbol</Th>
                    <Th>Side</Th>
                    <Th>Status</Th>
                    <Th>Qty</Th>
                    <Th>Entry</Th>
                    <Th>Exit</Th>
                    <Th>PnL</Th>
                  </tr>
                </thead>
                <tbody>
                  {trades.map((t) => (
                    <tr key={t.trade_id}>
                      <Td>{new Date(t.opened_at).toLocaleString()}</Td>
                      <Td>{t.symbol}</Td>
                      <Td>{t.side}</Td>
                      <Td>{t.status}</Td>
                      <Td>{num(t.quantity, 6)}</Td>
                      <Td>{num(t.entry_price, 6)}</Td>
                      <Td>{num(t.exit_price, 6)}</Td>
                      <Td>{num(t.pnl, 6)}</Td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section style={{ marginBottom: 20 }}>
            <h2 style={{ marginBottom: 8 }}>Phase 5 Checklist + Evidence</h2>
            <div style={panelStyle}>
              <ul style={{ margin: 0 }}>
                {checklist.map((item) => (
                  <li key={item.label} style={{ color: item.ok ? "#1d4d1d" : "#7a1f1f" }}>
                    [{item.ok ? "OK" : "PENDING"}] {item.label}
                  </li>
                ))}
              </ul>
              <p style={{ marginTop: 10, marginBottom: 0 }}>
                Evidence: fetched_at={data.source.fetchedAt}, trades={trades.length}, events={events.length}, services={services.length}
              </p>
            </div>
          </section>
        </>
      )}
    </main>
  );
}

function Card({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ border: "1px solid #ddd", borderRadius: 8, padding: 12, background: "#fafafa" }}>
      <div style={{ fontSize: 12, color: "#666", marginBottom: 4 }}>{label}</div>
      <div style={{ fontWeight: 700 }}>{value}</div>
    </div>
  );
}

function Th({ children }: { children: ReactNode }) {
  return <th style={{ textAlign: "left", borderBottom: "1px solid #ddd", padding: "8px 10px" }}>{children}</th>;
}

function Td({ children }: { children: ReactNode }) {
  return <td style={{ borderBottom: "1px solid #eee", padding: "8px 10px", verticalAlign: "top" }}>{children}</td>;
}

const tableStyle: CSSProperties = {
  width: "100%",
  borderCollapse: "collapse",
  border: "1px solid #ddd",
  borderRadius: 8,
  overflow: "hidden",
  background: "#fff",
};

const panelStyle: CSSProperties = {
  border: "1px solid #ddd",
  borderRadius: 8,
  padding: 12,
  background: "#fafafa",
};

const labelStyle: CSSProperties = {
  marginBottom: 6,
  fontSize: 12,
  color: "#444",
};

const inputStyle: CSSProperties = {
  width: "100%",
  height: 34,
  border: "1px solid #bbb",
  borderRadius: 6,
  padding: "0 10px",
  boxSizing: "border-box",
};

const btnStyle: CSSProperties = {
  height: 34,
  border: "1px solid #444",
  borderRadius: 6,
  background: "#fff",
  padding: "0 12px",
  cursor: "pointer",
};
