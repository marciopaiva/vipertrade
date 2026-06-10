'use client';

import { cn } from '@/lib/utils';
import { useDecisions } from '@/hooks/useDecisions';
import { DecisionCard } from '@/components/cockpit/DecisionCard';

const LONG_PB_CEILING = 0.85;
const SHORT_PB_FLOOR = 0.15;

export default function StrategyPage() {
  const { decisions, loading, error, live } = useDecisions();

  const entering = decisions.filter(d => d.action.startsWith('ENTER')).length;
  const guarded = decisions.filter(d => {
    const pb = d.consensus_bollinger_percent_b;
    return typeof pb === 'number' && (pb > LONG_PB_CEILING || pb < SHORT_PB_FLOOR);
  }).length;

  return (
    <div className="space-y-5">
      {/* header */}
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-bold tracking-tight text-foreground">
            <span className="relative flex h-2.5 w-2.5">
              {live && (
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent opacity-60" />
              )}
              <span
                className={cn(
                  'relative inline-flex h-2.5 w-2.5 rounded-full',
                  live ? 'bg-accent' : 'bg-muted-foreground'
                )}
              />
            </span>
            Strategy
            <span
              className={cn(
                'rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide',
                live ? 'bg-accent/15 text-accent' : 'bg-secondary text-muted-foreground'
              )}
            >
              {live ? 'live' : 'polling'}
            </span>
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Per-symbol multi-exchange consensus, entry guards, and the reason each
            symbol is entering or being held.
          </p>
        </div>
        <div className="flex items-center gap-4 font-mono text-sm tabular-nums">
          <span className="text-muted-foreground">
            <span className="text-foreground">{decisions.length}</span> symbols
          </span>
          <span className="text-muted-foreground">
            <span className="text-accent">{entering}</span> entering
          </span>
          <span className="text-muted-foreground">
            <span className="text-warn">{guarded}</span> guarded
          </span>
        </div>
      </div>

      {/* legend */}
      <div className="flex flex-wrap items-center gap-x-5 gap-y-1 text-[11px] text-muted-foreground">
        <span className="inline-flex items-center gap-1.5">
          <span className="h-2 w-3 rounded-sm bg-destructive/25" /> %B guard zone
          (≥{LONG_PB_CEILING} / ≤{SHORT_PB_FLOOR})
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span className="h-2 w-3 rounded-sm bg-warn/20" /> ADX &lt; 20 weak trend
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span className="h-2 w-3 rounded-sm bg-accent/20" /> ADX ≥ 25 strong trend
        </span>
      </div>

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && decisions.length === 0 ? (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <div
              key={i}
              className="h-72 animate-pulse rounded-xl border border-border bg-card"
            />
          ))}
        </div>
      ) : decisions.length === 0 ? (
        <div className="rounded-xl border border-border bg-card px-3 py-12 text-center text-sm text-muted-foreground">
          No decisions recorded yet — the strategy publishes one per symbol as
          market data flows in.
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
          {decisions.map(d => (
            <DecisionCard key={d.symbol} d={d} />
          ))}
        </div>
      )}
    </div>
  );
}
