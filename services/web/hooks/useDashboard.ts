'use client';

import { useState, useEffect, useCallback } from 'react';

// API base URL - uses relative path which works when accessed through same origin
const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || '';

interface UseDashboardOptions {
  refreshInterval?: number;
  enabled?: boolean;
}

interface UseDashboardReturn<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useDashboard<T = unknown>(
  endpoint: string,
  options: UseDashboardOptions = {}
): UseDashboardReturn<T> {
  const { refreshInterval = 5000, enabled = true } = options;
  
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!enabled) return;
    
    try {
      const url = endpoint.startsWith('http') ? endpoint : `${API_BASE_URL}${endpoint}`;
      const res = await fetch(url, { cache: 'no-store' });
      const raw = await res.text();
      const body = raw ? JSON.parse(raw) : null;
      
      if (!res.ok) {
        throw new Error(body?.message || `HTTP ${res.status}`);
      }
      
      if (!body) {
        throw new Error('Empty response');
      }
      
      setData(body);
      setError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, [endpoint, enabled]);

  useEffect(() => {
    fetchData();

    if (refreshInterval > 0 && enabled) {
      const interval = setInterval(fetchData, refreshInterval);
      return () => clearInterval(interval);
    }
    
    return undefined;
  }, [fetchData, refreshInterval, enabled]);

  return {
    data,
    loading,
    error,
    refresh: fetchData,
  };
}
