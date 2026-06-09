'use client';

import { useEffect, useState } from 'react';
import { fetchApi, endpoints } from '@/lib/api/endpoints';
import type { DecisionItem } from '@/types/trading';

/**
 * Polls the latest strategy decision per symbol (with the multi-exchange
 * consensus indicators) for the Strategy Cockpit.
 */
export function useDecisions(pollMs = 4000) {
  const [decisions, setDecisions] = useState<DecisionItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);

  useEffect(() => {
    let active = true;

    const load = async () => {
      try {
        const data = await fetchApi<{ items: DecisionItem[] }>(
          endpoints.decisions
        );
        if (!active) return;
        setDecisions(data?.items ?? []);
        setUpdatedAt(Date.now());
        setError(null);
      } catch (err) {
        if (active) {
          setError(
            err instanceof Error ? err.message : 'Failed to fetch decisions'
          );
        }
      } finally {
        if (active) setLoading(false);
      }
    };

    load();
    const id = setInterval(load, pollMs);
    return () => {
      active = false;
      clearInterval(id);
    };
  }, [pollMs]);

  return { decisions, loading, error, updatedAt };
}
