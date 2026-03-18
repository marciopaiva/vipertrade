'use client';

import { useEffect, useState } from 'react';
import { useTradingStore } from '@/stores/tradingStore';
import { fetchApi, endpoints } from '@/lib/api/endpoints';

export function useServiceHealth() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { services, setServices } = useTradingStore();

  useEffect(() => {
    const fetchHealth = async () => {
      try {
        setLoading(true);
        const data = await fetchApi(endpoints.dashboard);
        setServices(data?.services || []);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to fetch service health');
      } finally {
        setLoading(false);
      }
    };

    fetchHealth();

    // Poll every 30 seconds
    const interval = setInterval(fetchHealth, 30000);

    return () => clearInterval(interval);
  }, []);

  return { services, loading, error };
}
