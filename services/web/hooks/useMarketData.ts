'use client';

import { useEffect, useState } from 'react';
import { useTradingStore } from '@/stores/tradingStore';
import { getWebSocketClient } from '@/lib/websocket/client';
import { fetchApi, endpoints } from '@/lib/api/endpoints';

export function useMarketData() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { marketSignals, setMarketSignals } = useTradingStore();

  useEffect(() => {
    // Fetch initial data
    const fetchMarketData = async () => {
      try {
        setLoading(true);
        const data = await fetchApi(endpoints.marketSignals);
        setMarketSignals(data?.items || []);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to fetch market data');
      } finally {
        setLoading(false);
      }
    };

    fetchMarketData();

    // Setup WebSocket for real-time updates
    const ws = getWebSocketClient();
    ws.on('market_signal', (data) => {
      setMarketSignals([data, ...marketSignals.slice(0, 99)]);
    });

    return () => {
      ws.off('market_signal', () => {});
    };
  }, []);

  return { marketSignals, loading, error };
}
