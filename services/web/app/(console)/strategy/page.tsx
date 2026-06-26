'use client';

import { cn } from '@/lib/utils';
import { useDecisions } from '@/hooks/useDecisions';
import { useT } from '@/lib/i18n';
import { DecisionRow, ROW_GRID } from '@/components/cockpit/DecisionRow';

const LONG_PB_CEILING = 0.85;
const SHORT_PB_FLOOR = 0.15;

export default function StrategyPage() {
  const t = useT('strategy');
  const { decisions, loading, error, live } = useDecisions();

  const entering = decisions.filter(d => d.action.startsWith('ENTER')).length;
  const guarded = decisions.filter(d => {
    const pb = d.consensus_bollinger_percent_b;
    return typeof pb === 'number' && (pb > LONG_PB_CEILING || pb < SHORT_PB_FLOOR);
  }).length;

  // Entering symbols float to the top (the actionable ones), then alphabetical.
  const ordered = [...decisions].sort((a, b) => {
    const ae = a.action.startsWith('ENTER') ? 0 : 1;
    const be = b.action.startsWith('ENTER') ? 0 : 1;
    return ae - be || a.symbol.localeCompare(b.symbol);
  });

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
            {t('title')}
            <span
              className={cn(
                'rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide',
                live ? 'bg-accent/15 text-accent' : 'bg-secondary text-muted-foreground'
              )}
            >
              {live ? t('live') : t('polling')}
            </span>
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">{t('subtitle')}</p>
        </div>
        <div className="flex items-center gap-4 font-mono text-sm tabular-nums">
          <span className="text-muted-foreground">
            <span className="text-foreground">{decisions.length}</span>{' '}
            {t('symbols')}
          </span>
          <span className="text-muted-foreground">
            <span className="text-accent">{entering}</span> {t('entering')}
          </span>
          <span className="text-muted-foreground">
            <span className="text-warn">{guarded}</span> {t('guarded')}
          </span>
        </div>
      </div>

      {/* legend */}
      <div className="flex flex-wrap items-center gap-x-5 gap-y-1 text-[11px] text-muted-foreground">
        <span className="inline-flex items-center gap-1.5">
          <span className="h-2 w-3 rounded-sm bg-destructive/25" />{' '}
          {t('legendGuard', { ceil: LONG_PB_CEILING, floor: SHORT_PB_FLOOR })}
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span className="h-2 w-3 rounded-sm bg-warn/20" /> {t('legendWeak')}
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span className="h-2 w-3 rounded-sm bg-accent/20" /> {t('legendStrong')}
        </span>
      </div>

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && decisions.length === 0 ? (
        <div className="space-y-2">
          {Array.from({ length: 6 }).map((_, i) => (
            <div
              key={i}
              className="h-14 animate-pulse rounded-lg border border-border bg-card"
            />
          ))}
        </div>
      ) : decisions.length === 0 ? (
        <div className="rounded-xl border border-border bg-card px-3 py-12 text-center text-sm text-muted-foreground">
          {t('empty')}
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-border bg-card">
          {/* column header */}
          <div
            className={cn(
              ROW_GRID,
              'border-b border-border px-3 py-2 text-[10px] uppercase tracking-[0.15em] text-muted-foreground'
            )}
          >
            <span>{t('colSymbol')}</span>
            <span>{t('colState')}</span>
            <span>{t('colConsensus')}</span>
            <span>{t('colRsi')}</span>
            <span>{t('colPb')}</span>
            <span>{t('colAdx')}</span>
            <span>{t('colWhy')}</span>
          </div>
          {ordered.map(d => (
            <div key={d.symbol} id={`sym-${d.symbol}`} className="scroll-mt-24">
              <DecisionRow d={d} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
