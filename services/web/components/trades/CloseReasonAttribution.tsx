'use client';

import { useMemo } from 'react';
import { cn } from '@/lib/utils';
import type { Trade } from '@/types/trading';

function titleCase(value?: string | null) {
  if (!value) return 'Unknown';
  return value
    .replaceAll('_', ' ')
    .toLowerCase()
    .replace(/\b\w/g, c => c.toUpperCase());
}

function usd(value: number) {
  const sign = value > 0 ? '+' : value < 0 ? '−' : '';
  return `${sign}$${Math.abs(value).toFixed(2)}`;
}

interface ReasonStat {
  reason: string;
  count: number;
  net: number;
  wins: number;
}

/**
 * PnL attribution by close-reason — the lesson the backtest taught us, made
 * visible: trailing tends to be the edge, thesis/stop the bleed. Cards are
 * clickable to filter the ledger to that reason. Magnitude bars are scaled to
 * the largest |net| so the dominant contributor reads at a glance.
 */
export function CloseReasonAttribution({
  trades,
  activeReason,
  onSelectReason,
}: {
  trades: Trade[];
  activeReason: string | null;
  onSelectReason: (reason: string | null) => void;
}) {
  const stats = useMemo<ReasonStat[]>(() => {
    const byReason = new Map<string, ReasonStat>();
    for (const t of trades) {
      if (t.status !== 'closed') continue;
      const reason = t.close_reason || 'unknown';
      const s =
        byReason.get(reason) ?? { reason, count: 0, net: 0, wins: 0 };
      s.count += 1;
      s.net += t.pnl ?? 0;
      if ((t.pnl ?? 0) >= 0) s.wins += 1;
      byReason.set(reason, s);
    }
    return [...byReason.values()].sort((a, b) => b.net - a.net);
  }, [trades]);

  if (stats.length === 0) return null;

  const maxAbs = Math.max(...stats.map(s => Math.abs(s.net)), 1);

  return (
    <section className="space-y-2">
      <h2 className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        PnL by close reason
      </h2>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5">
        {stats.map(s => {
          const positive = s.net >= 0;
          const active = activeReason === s.reason;
          const winRate = s.count ? (s.wins / s.count) * 100 : 0;
          return (
            <button
              key={s.reason}
              type="button"
              onClick={() => onSelectReason(active ? null : s.reason)}
              aria-pressed={active}
              className={cn(
                'flex flex-col gap-2 rounded-lg border bg-card p-3 text-left transition-colors',
                active
                  ? 'border-primary/60 ring-1 ring-primary/40'
                  : 'border-border hover:border-primary/40'
              )}
            >
              <span className="truncate text-xs font-medium text-foreground">
                {titleCase(s.reason)}
              </span>
              <span
                className={cn(
                  'font-mono text-lg font-bold tabular-nums',
                  positive ? 'text-accent' : 'text-destructive'
                )}
              >
                {usd(s.net)}
              </span>
              <div className="h-1 w-full overflow-hidden rounded-full bg-secondary">
                <div
                  className={cn(
                    'h-full rounded-full',
                    positive ? 'bg-accent' : 'bg-destructive'
                  )}
                  style={{ width: `${(Math.abs(s.net) / maxAbs) * 100}%` }}
                />
              </div>
              <span className="font-mono text-[11px] tabular-nums text-muted-foreground">
                {s.count} {s.count === 1 ? 'trade' : 'trades'} · {winRate.toFixed(0)}% win
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
