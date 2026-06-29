'use client';

import { useEffect, useState } from 'react';
import { getWebSocketClient, type ConnectionStatus } from '@/lib/websocket/client';

export type LiveStatus = ConnectionStatus;

/**
 * Lightweight global liveness for the header pill: delegates to the shared
 * WebSocket singleton so there is only one connection in the whole app.
 */
export function useConnectionStatus(): {
  status: LiveStatus;
  lastMessageAt: number | null;
} {
  const [status, setStatus] = useState<ConnectionStatus>('connecting');
  const [lastMessageAt, setLastMessageAt] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    let cleanup: (() => void) | undefined;
    getWebSocketClient().then((ws) => {
      if (cancelled) return;
      setStatus(ws.status);
      if (ws.lastMessageTimestamp > 0) {
        setLastMessageAt(ws.lastMessageTimestamp);
      }
      const unsubStatus = ws.onStatusChange(setStatus);
      const unsubMessage = ws.on('message', () => setLastMessageAt(Date.now()));
      ws.connect();
      cleanup = () => {
        unsubStatus();
        unsubMessage();
      };
    });
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, []);

  return { status, lastMessageAt };
}
