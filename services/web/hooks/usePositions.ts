'use client';

import { useEffect, useState } from 'react';
import { useTradingStore } from '@/stores/tradingStore';
import { getWebSocketClient } from '@/lib/websocket/client';
import { fetchApi, endpoints } from '@/lib/api/endpoints';

export function usePositions() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { positions, setPositions } = useTradingStore();

  useEffect(() => {
    const fetchPositions = async () => {
      try {
        setLoading(true);
        const data = await fetchApi(endpoints.positions);
        setPositions(data?.items || []);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to fetch positions');
      } finally {
        setLoading(false);
      }
    };

    fetchPositions();

    // Setup WebSocket for real-time position updates
    const ws = getWebSocketClient();
    ws.on('position_update', (data) => {
      setPositions((prev: any[]) => {
        const index = prev.findIndex(p => p.trade_id === data.trade_id);
        if (index >= 0) {
          const updated = [...prev];
          updated[index] = data;
          return updated;
        }
        return [data, ...prev];
      });
    });

    return () => {
      ws.off('position_update', () => {});
    };
  }, []);

  return { positions, loading, error };
}
