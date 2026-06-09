'use client';

import { cn } from '@/lib/utils';
import type { DecisionItem } from '@/types/trading';
import { GaugeBar } from './GaugeBar';

const LONG_PB_CEILING = 0.85;
const SHORT_PB_FLOOR = 0.15;

function actionStyle(action: string) {
  if (action === 'ENTER_LONG')
    return 'bg-accent/15 text-accent border-accent/30';
  if (action === 'ENTER_SHORT')
    return 'bg-destructive/15 text-destructive border-destructive/30';
  return 'bg-secondary text-muted-foreground border-border';
}

function GuardChip({ label }: { label: string }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-destructive/40 bg-destructive/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-destructive">
      <span aria-hidden>⛔</span>
      {label}
    </span>
  );
}

export function ConsensusCard({ d }: { d: DecisionItem }) {
  const available = d.exchanges_available ?? 0;
  const bull = d.bullish_exchanges ?? 0;
  const bear = d.bearish_exchanges ?? 0;
  const neutral = Math.max(0, available - bull - bear);
  const total = Math.max(1, available);

  const pb = d.consensus_bollinger_percent_b;
  const rsi = d.consensus_rsi_14;
  const trend = d.consensus_trend_score ?? 0;
  const macd = d.consensus_macd_histogram ?? 0;

  const longBlocked = typeof pb === 'number' && pb > LONG_PB_CEILING;
  const shortBlocked = typeof pb === 'number' && pb < SHORT_PB_FLOOR;

  return (
    <div className="group rounded-lg border border-border bg-card p-4 transition-colors hover:border-primary/40">
      {/* header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-sm font-bold tracking-tight text-foreground">
            {d.symbol}
          </span>
          {typeof d.current_price === 'number' && (
            <span className="font-mono text-xs text-muted-foreground">
              {d.current_price.toLocaleString(undefined, {
                maximumFractionDigits: 6,
              })}
            </span>
          )}
        </div>
        <span
          className={cn(
            'rounded-md border px-2 py-0.5 text-[11px] font-semibold tracking-wide',
            actionStyle(d.action)
          )}
        >
          {d.action.replace('ENTER_', '')}
        </span>
      </div>

      {/* 3-exchange consensus strip */}
      <div className="mt-3">
        <div className="mb-1 flex items-center justify-between text-[11px] text-muted-foreground">
          <span className="uppercase tracking-wide">Consensus</span>
          <span className="font-mono">
            <span
              className={cn(
                d.consensus_side === 'bullish' && 'text-accent',
                d.consensus_side === 'bearish' && 'text-destructive'
              )}
            >
              {d.consensus_side ?? 'n/a'}
            </span>{' '}
            · {d.consensus_count ?? 0}/{available}
          </span>
        </div>
        <div className="flex h-2 w-full overflow-hidden rounded-full bg-secondary">
          <div
            className="bg-accent"
            style={{ width: `${(bull / total) * 100}%` }}
            title={`${bull} bullish`}
          />
          <div
            className="bg-muted"
            style={{ width: `${(neutral / total) * 100}%` }}
            title={`${neutral} neutral`}
          />
          <div
            className="bg-destructive"
            style={{ width: `${(bear / total) * 100}%` }}
            title={`${bear} bearish`}
          />
        </div>
      </div>

      {/* indicators */}
      <div className="mt-3 space-y-2.5">
        <GaugeBar
          label="RSI"
          value={rsi}
          min={0}
          max={100}
          format={v => v.toFixed(0)}
          zones={[
            { from: 0, to: 30, className: 'bg-primary/20' },
            { from: 70, to: 100, className: 'bg-primary/20' },
          ]}
        />
        <GaugeBar
          label="Bollinger %B"
          value={pb}
          min={-0.2}
          max={1.2}
          format={v => v.toFixed(2)}
          danger={longBlocked || shortBlocked}
          zones={[
            { from: -0.2, to: SHORT_PB_FLOOR, className: 'bg-destructive/25' },
            { from: LONG_PB_CEILING, to: 1.2, className: 'bg-destructive/25' },
          ]}
        />
      </div>

      {/* footer: trend / macd / guards */}
      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[11px]">
        <span className="text-muted-foreground">
          trend{' '}
          <span
            className={cn(
              'font-mono',
              trend > 0 ? 'text-accent' : trend < 0 ? 'text-destructive' : ''
            )}
          >
            {trend >= 0 ? '+' : ''}
            {trend.toFixed(3)}
          </span>
        </span>
        <span className="text-muted-foreground">
          macd{' '}
          <span
            className={cn(
              'font-mono',
              macd > 0 ? 'text-accent' : macd < 0 ? 'text-destructive' : ''
            )}
          >
            {macd >= 0 ? '+' : ''}
            {macd.toFixed(4)}
          </span>
        </span>
        <div className="ml-auto flex gap-1.5">
          {longBlocked && <GuardChip label="long block" />}
          {shortBlocked && <GuardChip label="short block" />}
        </div>
      </div>
    </div>
  );
}
