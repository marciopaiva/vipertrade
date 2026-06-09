'use client';

import { useDecisions } from '@/hooks/useDecisions';
import { ConsensusCard } from './ConsensusCard';

export function StrategyCockpit() {
  const { decisions, loading, error, updatedAt } = useDecisions(4000);

  const entries = decisions.filter(d => d.action.startsWith('ENTER')).length;

  return (
    <section className="space-y-4">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h2 className="flex items-center gap-2 text-lg font-semibold text-foreground">
            <span className="relative flex h-2.5 w-2.5">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent opacity-60" />
              <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-accent" />
            </span>
            Strategy Cockpit
          </h2>
          <p className="text-xs text-muted-foreground">
            Live multi-exchange consensus &amp; entry-guard state per symbol
          </p>
        </div>
        <div className="text-right text-xs text-muted-foreground">
          <div>
            <span className="font-mono text-foreground">
              {decisions.length}
            </span>{' '}
            symbols · <span className="font-mono text-accent">{entries}</span>{' '}
            entering
          </div>
          {updatedAt && (
            <div>updated {new Date(updatedAt).toLocaleTimeString()}</div>
          )}
        </div>
      </header>

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && decisions.length === 0 ? (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <div
              key={i}
              className="h-52 animate-pulse rounded-lg border border-border bg-card"
            />
          ))}
        </div>
      ) : decisions.length === 0 ? (
        <div className="rounded-md border border-border bg-card px-3 py-8 text-center text-sm text-muted-foreground">
          No decisions recorded yet.
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {decisions.map(d => (
            <ConsensusCard key={d.symbol} d={d} />
          ))}
        </div>
      )}
    </section>
  );
}
