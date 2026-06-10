'use client';

import Link from 'next/link';
import { cn } from '@/lib/utils';
import type { DecisionItem } from '@/types/trading';

const LONG_PB_CEILING = 0.85;
const SHORT_PB_FLOOR = 0.15;

function isGuarded(d: DecisionItem) {
  const pb = d.consensus_bollinger_percent_b;
  return typeof pb === 'number' && (pb > LONG_PB_CEILING || pb < SHORT_PB_FLOOR);
}

/** Tile tint: hue from consensus side, opacity from consensus strength. */
function tileTone(d: DecisionItem) {
  const side = d.consensus_side ?? 'neutral';
  const count = d.consensus_count ?? 0;
  const available = Math.max(1, d.exchanges_available ?? 0);
  const strength = Math.max(0.12, Math.min(1, count / available));
  if (side === 'bullish')
    return { backgroundColor: `rgba(0, 255, 136, ${0.06 + strength * 0.14})` };
  if (side === 'bearish')
    return { backgroundColor: `rgba(239, 68, 68, ${0.06 + strength * 0.14})` };
  return { backgroundColor: 'rgba(148, 163, 184, 0.06)' };
}

function PulseTile({ d }: { d: DecisionItem }) {
  const entering = d.action.startsWith('ENTER');
  const guarded = isGuarded(d);
  const available = Math.max(1, d.exchanges_available ?? 0);
  const bull = d.bullish_exchanges ?? 0;
  const bear = d.bearish_exchanges ?? 0;
  const neutral = Math.max(0, available - bull - bear);

  return (
    <Link
      href={`/strategy#sym-${d.symbol}`}
      style={tileTone(d)}
      className={cn(
        'group flex flex-col gap-2 rounded-lg border p-2.5 transition-colors',
        entering
          ? d.action === 'ENTER_LONG'
            ? 'border-accent/40'
            : 'border-destructive/40'
          : guarded
            ? 'border-warn/40'
            : 'border-border hover:border-primary/40'
      )}
    >
      <div className="flex items-center justify-between gap-1">
        <span className="truncate text-xs font-bold text-foreground">
          {d.symbol}
        </span>
        {guarded && (
          <span
            title="entry guard in play (%B band extreme)"
            className="h-1.5 w-1.5 shrink-0 rounded-full bg-warn"
          />
        )}
      </div>

      {/* consensus heatmap bar */}
      <div className="flex h-1.5 w-full overflow-hidden rounded-full bg-secondary">
        <div className="bg-accent" style={{ width: `${(bull / available) * 100}%` }} />
        <div className="bg-muted" style={{ width: `${(neutral / available) * 100}%` }} />
        <div className="bg-destructive" style={{ width: `${(bear / available) * 100}%` }} />
      </div>

      <span
        className={cn(
          'text-[10px] font-semibold uppercase tracking-wide',
          d.action === 'ENTER_LONG'
            ? 'text-accent'
            : d.action === 'ENTER_SHORT'
              ? 'text-destructive'
              : 'text-muted-foreground'
        )}
      >
        {entering ? d.action.replace('ENTER_', '') : guarded ? 'Guarded' : 'Hold'}
      </span>
    </Link>
  );
}

/**
 * Compact Console summary of the strategy: per-symbol consensus heatmap +
 * ENTER-ready / held-by-guard counts. The full per-symbol detail lives on
 * /strategy — every tile deep-links there. Driven by the Console's existing
 * useDecisions subscription (passed in) so it adds no second WS connection.
 */
export function StrategyPulse({
  decisions,
  live = false,
  loading = false,
}: {
  decisions: DecisionItem[];
  live?: boolean;
  loading?: boolean;
}) {
  const sorted = [...decisions].sort((a, b) => a.symbol.localeCompare(b.symbol));
  const entering = sorted.filter(d => d.action.startsWith('ENTER')).length;
  const guarded = sorted.filter(isGuarded).length;

  return (
    <section className="space-y-3">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <h2 className="flex items-center gap-2 text-base font-semibold text-foreground">
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
            Strategy Pulse
          </h2>
          <Link
            href="/strategy"
            className="text-xs text-muted-foreground transition-colors hover:text-viper-cyan"
          >
            full view →
          </Link>
        </div>
        <div className="font-mono text-xs tabular-nums text-muted-foreground">
          <span className="text-foreground">{sorted.length}</span> symbols ·{' '}
          <span className="text-accent">{entering}</span> ENTER-ready ·{' '}
          <span className="text-warn">{guarded}</span> held by guard
        </div>
      </header>

      {loading && sorted.length === 0 ? (
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {Array.from({ length: 6 }).map((_, i) => (
            <div
              key={i}
              className="h-20 animate-pulse rounded-lg border border-border bg-card"
            />
          ))}
        </div>
      ) : sorted.length === 0 ? (
        <div className="rounded-lg border border-border bg-card px-3 py-8 text-center text-sm text-muted-foreground">
          No decisions recorded yet — the strategy publishes one per symbol as
          market data flows in.
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {sorted.map(d => (
            <PulseTile key={d.symbol} d={d} />
          ))}
        </div>
      )}
    </section>
  );
}
