'use client';

import { useState } from 'react';
import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';
import { SectionCard } from '@/components/ui/SectionCard';

export type VariantClass = 'alpha' | 'exposure';

export type GridVariant = {
  axis: string;
  path: string;
  value: string;
  class: VariantClass;
  delta_net_pnl: number;
  net_pnl: number;
  closed: number;
  wins: number;
  losses: number;
};

export type SymbolPerf = {
  symbol: string;
  trades: number;
  net_pnl: number;
  wins: number;
  win_rate_pct: number;
  enabled: boolean;
};

export type Substitution = {
  drop_candidate: string | null;
  drop_reason: string | null;
  pool: string[];
};

export type Baseline = {
  net_pnl: number;
  closed: number;
  wins: number;
  losses: number;
  win_rate_pct: number;
  by_reason: Record<string, [number, number]>;
};

export type TuningResponse = {
  corpus_ticks: number;
  baseline: Baseline;
  variants: GridVariant[];
  by_symbol: SymbolPerf[];
  substitution: Substitution;
  recommended: GridVariant | null;
};

export const tone = (v: number) =>
  v > 0 ? 'text-accent' : v < 0 ? 'text-destructive' : 'text-muted-foreground';

export function ClassBadge({ klass }: { klass: VariantClass }) {
  return (
    <span
      className={cn(
        'rounded-md border px-1.5 py-0.5 text-[10px] uppercase tracking-wide',
        klass === 'alpha'
          ? 'border-accent/40 bg-accent/10 text-accent'
          : 'border-amber-500/40 bg-amber-500/10 text-amber-500',
      )}
    >
      {klass}
    </span>
  );
}

export function SubstitutionCard({ sub }: { sub: Substitution }) {
  const t = useT('whatif');
  if (!sub.drop_candidate && sub.pool.length === 0) return null;
  return (
    <SectionCard title={t('subTitle')}>
      {sub.drop_candidate ? (
        <p className="text-sm text-foreground">
          {t('subWorst', { symbol: sub.drop_candidate })}
          {sub.drop_reason && <span className="text-muted-foreground"> — {sub.drop_reason}</span>}
        </p>
      ) : (
        <p className="text-sm text-muted-foreground">{t('subNone')}</p>
      )}
      {sub.pool.length > 0 && (
        <p className="mt-2 text-xs text-muted-foreground">
          {t('subPool')}{' '}
          {sub.pool.map(s => (
            <span
              key={s}
              className="mr-1 inline-block rounded border border-border bg-secondary/40 px-1.5 py-0.5 font-mono text-foreground"
            >
              {s}
            </span>
          ))}
        </p>
      )}
    </SectionCard>
  );
}

export type TuningState = {
  data: TuningResponse | null;
  loading: boolean;
  error: string | null;
  run: () => Promise<void>;
};

// Shared on-demand fetch state for the deterministic /analyze/tuning grid (What-if tab).
export function useTuning(): TuningState {
  const [data, setData] = useState<TuningResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run() {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch('/api/analysis/tuning', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ limit: 60000 }),
      });
      const json = await res.json();
      if (!res.ok) throw new Error(json?.message || `HTTP ${res.status}`);
      setData(json as TuningResponse);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  return { data, loading, error, run };
}
