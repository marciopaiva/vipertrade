'use client';

import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';
import type { DecisionItem } from '@/types/trading';
import { MiniGauge } from './MiniGauge';

type T = ReturnType<typeof useT<'strategy'>>;

const LONG_PB_CEILING = 0.85;
const SHORT_PB_FLOOR = 0.15;
const ADX_WEAK = 20;
const ADX_STRONG = 25;

/** Translate a consensus side label; falls back to n/a for missing values. */
function sideLabel(t: T, side?: string | null): string {
  if (side === 'bullish') return t('sideBullish');
  if (side === 'bearish') return t('sideBearish');
  if (side === 'neutral') return t('sideNeutral');
  return t('na');
}

// Shared grid template so the header and every row align. Scrolls on narrow
// screens via the parent's overflow-x-auto.
export const ROW_GRID =
  'grid grid-cols-[104px_92px_136px_116px_124px_112px_minmax(180px,1fr)] items-center gap-x-3';

function actionStyle(action: string) {
  if (action === 'ENTER_LONG') return 'bg-accent/15 text-accent border-accent/30';
  if (action === 'ENTER_SHORT')
    return 'bg-destructive/15 text-destructive border-destructive/30';
  return 'bg-secondary text-muted-foreground border-border';
}

/** Plain-language "why" — %B is a real gate, ADX is trend-strength context. */
function explain(
  d: DecisionItem,
  t: T
): { kind: 'enter' | 'hold'; text: string } {
  const pb = d.consensus_bollinger_percent_b;
  const adx = d.consensus_adx_14;
  const side = d.consensus_side ?? 'neutral';
  const count = d.consensus_count ?? 0;

  if (d.action.startsWith('ENTER')) {
    const dir = d.action.replace('ENTER_', '').toLowerCase();
    return {
      kind: 'enter',
      text: t('whyEnter', { dir, side: sideLabel(t, side) }),
    };
  }
  const reasons: string[] = [];
  if (side === 'neutral' || count === 0) reasons.push(t('rNoConsensus'));
  if (typeof pb === 'number' && pb > LONG_PB_CEILING)
    reasons.push(
      t('rPbLong', { pb: pb.toFixed(2), ceil: LONG_PB_CEILING })
    );
  if (typeof pb === 'number' && pb < SHORT_PB_FLOOR)
    reasons.push(
      t('rPbShort', { pb: pb.toFixed(2), floor: SHORT_PB_FLOOR })
    );
  if (typeof adx === 'number' && adx < ADX_WEAK)
    reasons.push(t('rWeakTrend', { adx: adx.toFixed(0) }));
  return {
    kind: 'hold',
    text: reasons.length
      ? t('whyHolding', { reasons: reasons.join('; ') })
      : t('whyHoldingDefault', { side: sideLabel(t, side) }),
  };
}

export function DecisionRow({ d }: { d: DecisionItem }) {
  const t = useT('strategy');
  const available = d.exchanges_available ?? 0;
  const bull = d.bullish_exchanges ?? 0;
  const bear = d.bearish_exchanges ?? 0;
  const neutral = Math.max(0, available - bull - bear);
  const total = Math.max(1, available);

  const pb = d.consensus_bollinger_percent_b;
  const rsi = d.consensus_rsi_14;
  const adx = d.consensus_adx_14;

  const pbTone =
    typeof pb === 'number' && (pb > LONG_PB_CEILING || pb < SHORT_PB_FLOOR)
      ? 'danger'
      : undefined;
  const adxTone =
    typeof adx === 'number' && adx < ADX_WEAK ? 'warn' : undefined;
  const why = explain(d, t);

  return (
    <div
      className={cn(
        ROW_GRID,
        'border-b border-border/50 px-3 py-2.5 text-sm transition-colors last:border-0 hover:bg-secondary/30'
      )}
    >
      {/* symbol + price */}
      <div className="min-w-0">
        <div className="truncate font-bold tracking-tight text-foreground">
          {d.symbol}
        </div>
        {typeof d.current_price === 'number' && (
          <div className="font-mono text-[11px] tabular-nums text-muted-foreground">
            {d.current_price.toLocaleString(undefined, {
              maximumFractionDigits: 6,
            })}
          </div>
        )}
      </div>

      {/* state */}
      <span
        className={cn(
          'inline-flex justify-center rounded-md border px-2 py-1 text-[11px] font-semibold tracking-wide',
          actionStyle(d.action)
        )}
      >
        {d.action.replace('ENTER_', '').replace('_', ' ')}
      </span>

      {/* consensus */}
      <div className="min-w-0">
        <div className="mb-1 truncate font-mono text-[11px] tabular-nums">
          <span
            className={cn(
              d.consensus_side === 'bullish' && 'text-accent',
              d.consensus_side === 'bearish' && 'text-destructive',
              !d.consensus_side && 'text-muted-foreground'
            )}
          >
            {sideLabel(t, d.consensus_side)}
          </span>{' '}
          <span className="text-muted-foreground">
            {d.consensus_count ?? 0}/{available}
          </span>
        </div>
        <div className="flex h-1.5 w-full overflow-hidden rounded-full bg-secondary">
          <div className="bg-accent" style={{ width: `${(bull / total) * 100}%` }} />
          <div className="bg-muted" style={{ width: `${(neutral / total) * 100}%` }} />
          <div
            className="bg-destructive"
            style={{ width: `${(bear / total) * 100}%` }}
          />
        </div>
      </div>

      {/* indicators */}
      <MiniGauge value={rsi} min={0} max={100} format={v => v.toFixed(0)} />
      <MiniGauge
        value={pb}
        min={-0.2}
        max={1.2}
        format={v => v.toFixed(2)}
        tone={pbTone}
        zones={[
          { from: -0.2, to: SHORT_PB_FLOOR, className: 'bg-destructive/25' },
          { from: LONG_PB_CEILING, to: 1.2, className: 'bg-destructive/25' },
        ]}
      />
      <MiniGauge
        value={adx}
        min={0}
        max={50}
        format={v => v.toFixed(0)}
        tone={adxTone}
        zones={[
          { from: 0, to: ADX_WEAK, className: 'bg-warn/20' },
          { from: ADX_STRONG, to: 50, className: 'bg-accent/20' },
        ]}
      />

      {/* why (truncated; full on hover) */}
      <div
        className={cn(
          'flex items-center gap-1.5 truncate text-xs',
          why.kind === 'enter' ? 'text-accent' : 'text-muted-foreground'
        )}
        title={why.text}
      >
        <span aria-hidden className="shrink-0">
          {why.kind === 'enter' ? '▸' : '⏸'}
        </span>
        <span className="truncate">{why.text}</span>
      </div>
    </div>
  );
}
