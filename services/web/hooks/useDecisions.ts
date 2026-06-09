'use client';

import { useEffect, useRef, useState } from 'react';
import { fetchApi, endpoints } from '@/lib/api/endpoints';
import type { DecisionItem } from '@/types/trading';

/** WS stream URL: the api exposes /ws on host :8443 (kind maps 8443 -> api).
 *  Derived from the current host so it works wherever the dashboard is served.
 *  Override with NEXT_PUBLIC_WS_URL. */
function wsUrl(): string {
  const override = process.env.NEXT_PUBLIC_WS_URL;
  if (override && !override.includes(':8080')) return override;
  if (typeof window === 'undefined') return '';
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${window.location.hostname}:8443/ws`;
}

function num(v: unknown): number | null | undefined {
  return typeof v === 'number' && Number.isFinite(v) ? v : undefined;
}

/**
 * Strategy Cockpit data: latest decision per symbol. Loads once over REST,
 * then updates by WebSocket push — market_data events refresh the consensus
 * indicators, decision events refresh the action. A slow REST poll is kept as
 * a safety net if the socket drops.
 */
export function useDecisions() {
  const [bySymbol, setBySymbol] = useState<Record<string, DecisionItem>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [live, setLive] = useState(false);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  // Initial load + slow safety poll.
  useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const data = await fetchApi<{ items: DecisionItem[] }>(
          endpoints.decisions
        );
        if (!active) return;
        setBySymbol((prev) => {
          const next = { ...prev };
          for (const it of data?.items ?? []) next[it.symbol] = it;
          return next;
        });
        setUpdatedAt(Date.now());
        setError(null);
      } catch (err) {
        if (active)
          setError(
            err instanceof Error ? err.message : 'Failed to fetch decisions'
          );
      } finally {
        if (active) setLoading(false);
      }
    };
    load();
    const id = setInterval(load, 30000);
    return () => {
      active = false;
      clearInterval(id);
    };
  }, []);

  // WebSocket push.
  useEffect(() => {
    const url = wsUrl();
    if (!url) return;
    let active = true;
    let reconnect: ReturnType<typeof setTimeout> | null = null;

    const apply = (raw: string) => {
      let msg: any;
      try {
        msg = JSON.parse(raw);
      } catch {
        return;
      }
      // market_data event => refresh consensus indicators
      const sig = msg?.signal;
      if (sig?.symbol) {
        setBySymbol((prev) => {
          const cur = prev[sig.symbol] ?? {
            symbol: sig.symbol,
            action: 'HOLD',
            executed_at: new Date().toISOString(),
          };
          return {
            ...prev,
            [sig.symbol]: {
              ...cur,
              consensus_side: sig.consensus_side ?? cur.consensus_side,
              consensus_count: num(sig.consensus_count) ?? cur.consensus_count,
              exchanges_available:
                num(sig.exchanges_available) ?? cur.exchanges_available,
              bullish_exchanges:
                num(sig.bullish_exchanges) ?? cur.bullish_exchanges,
              bearish_exchanges:
                num(sig.bearish_exchanges) ?? cur.bearish_exchanges,
              consensus_rsi_14:
                num(sig.consensus_rsi_14) ?? cur.consensus_rsi_14,
              consensus_bollinger_percent_b:
                num(sig.consensus_bollinger_percent_b) ??
                cur.consensus_bollinger_percent_b,
              consensus_trend_score:
                num(sig.consensus_trend_score) ?? cur.consensus_trend_score,
              consensus_macd_histogram:
                num(sig.consensus_macd_histogram) ??
                cur.consensus_macd_histogram,
              current_price: num(sig.current_price) ?? cur.current_price,
              executed_at: new Date().toISOString(),
            },
          };
        });
        setUpdatedAt(Date.now());
        return;
      }
      // decision event => refresh action
      const action = msg?.decision?.action ?? msg?.action;
      const symbol = msg?.symbol ?? msg?.decision?.symbol;
      if (action && symbol) {
        setBySymbol((prev) =>
          prev[symbol]
            ? { ...prev, [symbol]: { ...prev[symbol], action } }
            : prev
        );
        setUpdatedAt(Date.now());
      }
    };

    const connect = () => {
      if (!active) return;
      try {
        const ws = new WebSocket(url);
        wsRef.current = ws;
        ws.onopen = () => active && setLive(true);
        ws.onmessage = (e) => apply(e.data as string);
        ws.onclose = () => {
          if (!active) return;
          setLive(false);
          reconnect = setTimeout(connect, 5000);
        };
        ws.onerror = () => ws.close();
      } catch {
        reconnect = setTimeout(connect, 5000);
      }
    };
    connect();

    return () => {
      active = false;
      if (reconnect) clearTimeout(reconnect);
      wsRef.current?.close();
    };
  }, []);

  // Stable alphabetical order by symbol so live WS updates don't reshuffle cards.
  const decisions = Object.values(bySymbol).sort((a, b) =>
    a.symbol.localeCompare(b.symbol)
  );

  return { decisions, loading, error, updatedAt, live };
}
