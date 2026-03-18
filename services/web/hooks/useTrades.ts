'use client';

import { useEffect, useState } from 'react';
import { useTradingStore } from '@/stores/tradingStore';
import { fetchApi, endpoints } from '@/lib/api/endpoints';

export function useTrades() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { trades, setTrades } = useTradingStore();

  useEffect(() => {
    const fetchTrades = async () => {
      try {
        setLoading(true);
        const data = await fetchApi(endpoints.trades);
        setTrades(data?.items || []);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to fetch trades');
      } finally {
        setLoading(false);
      }
    };

    fetchTrades();
  }, []);

  return { trades, loading, error };
}
