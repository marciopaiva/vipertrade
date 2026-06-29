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
        const data: any = await fetchApi(endpoints.marketSignals);
        setMarketSignals(data?.items || []);
        setError(null);
      } catch (err) {
        setError(
          err instanceof Error ? err.message : 'Failed to fetch market data'
        );
      } finally {
        setLoading(false);
      }
    };

    fetchMarketData();

    // Setup WebSocket for real-time updates
    let cancelled = false;
    let cleanup: (() => void) | undefined;
    getWebSocketClient().then((ws) => {
      if (cancelled) return;
      const handleMarketSignal = (data: any) => {
        const currentSignals = useTradingStore.getState().marketSignals;
        setMarketSignals([data, ...currentSignals.slice(0, 99)]);
      };
      ws.on('market_signal', handleMarketSignal);
      cleanup = () => {
        ws.off('market_signal', handleMarketSignal);
      };
    });

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [setMarketSignals]);

  return { marketSignals, loading, error };
}
