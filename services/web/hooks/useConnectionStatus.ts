'use client';

import { useEffect, useRef, useState } from 'react';

export type LiveStatus = 'connecting' | 'live' | 'stale' | 'down';

/** Same /ws endpoint the cockpit uses: api on host :8443 (kind maps 8443 -> api).
 *  Derived from the current host. Override with NEXT_PUBLIC_WS_URL. */
function wsUrl(): string {
  const override = process.env.NEXT_PUBLIC_WS_URL;
  if (override && !override.includes(':8080')) return override;
  if (typeof window === 'undefined') return '';
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${window.location.hostname}:8443/ws`;
}

// A live connection that hasn't pushed anything for this long is "stale".
const STALE_AFTER_MS = 20_000;

/**
 * Lightweight global liveness for the header pill: tracks the WebSocket
 * connection and how fresh the last push was. Opens one dedicated socket and
 * only reads message arrival times (no payload processing).
 */
export function useConnectionStatus(): {
  status: LiveStatus;
  lastMessageAt: number | null;
} {
  const [connected, setConnected] = useState(false);
  const [lastMessageAt, setLastMessageAt] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const wsRef = useRef<WebSocket | null>(null);

  // Connect (with reconnect), mirroring the cockpit's WS lifecycle.
  useEffect(() => {
    const url = wsUrl();
    if (!url) return;
    let active = true;
    let reconnect: ReturnType<typeof setTimeout> | null = null;

    const connect = () => {
      if (!active) return;
      try {
        const ws = new WebSocket(url);
        wsRef.current = ws;
        ws.onopen = () => active && setConnected(true);
        ws.onmessage = () => active && setLastMessageAt(Date.now());
        ws.onclose = () => {
          if (!active) return;
          setConnected(false);
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

  // Tick so "stale" is detected even when no messages arrive.
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 5000);
    return () => clearInterval(id);
  }, []);

  let status: LiveStatus = 'connecting';
  if (!connected) {
    status = lastMessageAt === null ? 'connecting' : 'down';
  } else if (lastMessageAt !== null && now - lastMessageAt > STALE_AFTER_MS) {
    status = 'stale';
  } else {
    status = 'live';
  }

  return { status, lastMessageAt };
}
