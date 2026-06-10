'use client';

import { cn } from '@/lib/utils';
import type { DecisionItem } from '@/types/trading';
import { GaugeBar } from './GaugeBar';

// Entry guards the web knows for certain (mirrors the strategy %B gate).
const LONG_PB_CEILING = 0.85;
const SHORT_PB_FLOOR = 0.15;
// Conventional ADX (Wilder) bands for trend strength: <20 weak/no trend, >25 strong.
const ADX_WEAK = 20;
const ADX_STRONG = 25;

function actionStyle(action: string) {
  if (action === 'ENTER_LONG')
    return 'bg-accent/15 text-accent border-accent/30';
  if (action === 'ENTER_SHORT')
    return 'bg-destructive/15 text-destructive border-destructive/30';
  return 'bg-secondary text-muted-foreground border-border';
}

/**
 * Plain-language "why" for a symbol's decision — the auditability promise made
 * visible. %B is an actual gate (known thresholds); ADX is shown as trend-
 * strength context (conventional bands), not asserted as the blocking gate.
 */
function explain(d: DecisionItem): { kind: 'enter' | 'hold'; text: string } {
  const pb = d.consensus_bollinger_percent_b;
  const adx = d.consensus_adx_14;
  const side = d.consensus_side ?? 'neutral';
  const count = d.consensus_count ?? 0;

  if (d.action.startsWith('ENTER')) {
    const dir = d.action.replace('ENTER_', '').toLowerCase();
    return {
      kind: 'enter',
      text: `Entering ${dir} — consensus ${side}, entry guards clear.`,
    };
  }

  const reasons: string[] = [];
  if (side === 'neutral' || count === 0) reasons.push('no directional consensus');
  if (typeof pb === 'number' && pb > LONG_PB_CEILING)
    reasons.push(`%B ${pb.toFixed(2)} > ${LONG_PB_CEILING} guards longs`);
  if (typeof pb === 'number' && pb < SHORT_PB_FLOOR)
    reasons.push(`%B ${pb.toFixed(2)} < ${SHORT_PB_FLOOR} guards shorts`);
  if (typeof adx === 'number' && adx < ADX_WEAK)
    reasons.push(`weak trend (ADX ${adx.toFixed(0)})`);

  return {
    kind: 'hold',
    text: reasons.length
      ? `Holding — ${reasons.join('; ')}.`
      : `Holding — consensus ${side}, conditions not yet aligned.`,
  };
}

export function DecisionCard({ d }: { d: DecisionItem }) {
  const available = d.exchanges_available ?? 0;
  const bull = d.bullish_exchanges ?? 0;
  const bear = d.bearish_exchanges ?? 0;
  const neutral = Math.max(0, available - bull - bear);
  const total = Math.max(1, available);

  const pb = d.consensus_bollinger_percent_b;
  const rsi = d.consensus_rsi_14;
  const adx = d.consensus_adx_14;
  const trend = d.consensus_trend_score ?? 0;

  const longBlocked = typeof pb === 'number' && pb > LONG_PB_CEILING;
  const shortBlocked = typeof pb === 'number' && pb < SHORT_PB_FLOOR;
  const why = explain(d);

  return (
    <div className="flex flex-col rounded-xl border border-border bg-card p-5 transition-colors hover:border-primary/40">
      {/* header */}
      <div className="flex items-center justify-between">
        <div className="flex items-baseline gap-2">
          <span className="text-base font-bold tracking-tight text-foreground">
            {d.symbol}
          </span>
          {typeof d.current_price === 'number' && (
            <span className="font-mono text-xs tabular-nums text-muted-foreground">
              {d.current_price.toLocaleString(undefined, {
                maximumFractionDigits: 6,
              })}
            </span>
          )}
        </div>
        <span
          className={cn(
            'rounded-md border px-2.5 py-1 text-xs font-semibold tracking-wide',
            actionStyle(d.action)
          )}
        >
          {d.action.replace('ENTER_', '').replace('_', ' ')}
        </span>
      </div>

      {/* consensus strip */}
      <div className="mt-4">
        <div className="mb-1 flex items-center justify-between text-[11px] text-muted-foreground">
          <span className="uppercase tracking-wide">Consensus</span>
          <span className="font-mono tabular-nums">
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
          <div className="bg-accent" style={{ width: `${(bull / total) * 100}%` }} />
          <div className="bg-muted" style={{ width: `${(neutral / total) * 100}%` }} />
          <div className="bg-destructive" style={{ width: `${(bear / total) * 100}%` }} />
        </div>
      </div>

      {/* indicators */}
      <div className="mt-4 space-y-3">
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
        <GaugeBar
          label="ADX · trend strength"
          value={adx}
          min={0}
          max={50}
          format={v => v.toFixed(0)}
          zones={[
            { from: 0, to: ADX_WEAK, className: 'bg-warn/20' },
            { from: ADX_STRONG, to: 50, className: 'bg-accent/20' },
          ]}
        />
      </div>

      {/* why */}
      <div
        className={cn(
          'mt-4 flex items-start gap-2 rounded-lg border px-3 py-2 text-xs',
          why.kind === 'enter'
            ? 'border-accent/30 bg-accent/10 text-accent'
            : 'border-border bg-secondary/40 text-muted-foreground'
        )}
      >
        <span aria-hidden className="mt-px">
          {why.kind === 'enter' ? '▸' : '⏸'}
        </span>
        <span>{why.text}</span>
      </div>

      {/* footer */}
      <div className="mt-3 flex items-center gap-x-4 text-[11px] text-muted-foreground">
        <span>
          trend{' '}
          <span
            className={cn(
              'font-mono tabular-nums',
              trend > 0 ? 'text-accent' : trend < 0 ? 'text-destructive' : ''
            )}
          >
            {trend >= 0 ? '+' : ''}
            {trend.toFixed(3)}
          </span>
        </span>
        {typeof adx === 'number' && (
          <span>
            ADX{' '}
            <span className="font-mono tabular-nums text-foreground">
              {adx.toFixed(1)}
            </span>{' '}
            {adx >= ADX_STRONG ? 'strong' : adx < ADX_WEAK ? 'weak' : 'emerging'}
          </span>
        )}
      </div>
    </div>
  );
}
