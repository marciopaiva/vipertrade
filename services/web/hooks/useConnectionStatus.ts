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
  const ws = getWebSocketClient();
  const [status, setStatus] = useState<ConnectionStatus>(ws.status);
  const [lastMessageAt, setLastMessageAt] = useState<number | null>(
    ws.lastMessageTimestamp > 0 ? ws.lastMessageTimestamp : null
  );

  useEffect(() => {
    const unsubStatus = ws.onStatusChange(setStatus);
    const unsubMessage = ws.on('message', () => setLastMessageAt(Date.now()));
    ws.connect();
    return () => {
      unsubStatus();
      unsubMessage();
    };
    // ws is the module-level singleton — stable reference, safe to exclude.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { status, lastMessageAt };
}
