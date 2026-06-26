'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { useLocale, useT, formatPrice, formatPct, formatUsd, formatNumber } from '@/lib/i18n';

interface MarketSignal {
  symbol: string;
  current_price: number;
  bybit_price?: number;
}

interface Position {
  trade_id: string;
  symbol: string;
  side: string;
  quantity: number;
  notional_usdt: number;
  entry_price: number;
  stop_loss_price?: number | null;
  trailing_stop_activated?: boolean;
  trailing_stop_peak_price?: number | null;
  trailing_stop_final_distance_pct?: number | null;
  trailing_activation_price?: number | null;
  break_even_price?: number | null;
}

interface PositionGaugeProps {
  positions: Position[];
  marketSignals?: MarketSignal[];
  guardedSetups?: number;
  className?: string;
}

/**
 * One open position as a horizontal "risk rail": left = stop (danger), right =
 * peak (best favorable price), filled left→right up to the current mark and
 * colored by unrealized PnL. The TP trigger (trailing activation) is marked with
 * an amber flag; once price crosses it the over-trigger profit is highlighted in
 * amber on the bar. There is no fixed take-profit (trailing strategy), so the rail
 * is oriented by *favorability* — right = more profit — which makes longs and
 * shorts read the same way (right = good, left = toward the stop).
 */
function PositionRow({ p, signal }: { p: Position; signal?: MarketSignal }) {
  const t = useT('positions');
  const locale = useLocale();
  const isLong = p.side.toLowerCase() === 'long';
  const entry =
    p.entry_price || (p.quantity > 0 ? p.notional_usdt / p.quantity : 0);
  const mark = signal?.bybit_price ?? signal?.current_price ?? entry;

  const trailing = Boolean(p.trailing_stop_activated);
  const stop =
    trailing &&
    typeof p.trailing_stop_peak_price === 'number' &&
    typeof p.trailing_stop_final_distance_pct === 'number'
      ? isLong
        ? p.trailing_stop_peak_price * (1 - p.trailing_stop_final_distance_pct)
        : p.trailing_stop_peak_price * (1 + p.trailing_stop_final_distance_pct)
      : typeof p.stop_loss_price === 'number'
        ? p.stop_loss_price
        : isLong
          ? entry * 0.988
          : entry * 1.012;
  const peak =
    typeof p.trailing_stop_peak_price === 'number'
      ? p.trailing_stop_peak_price
      : isLong
        ? Math.max(mark, entry)
        : Math.min(mark, entry);

  // TP trigger = the price at which the trailing stop arms (no fixed TP).
  const tpTrigger =
    typeof p.trailing_activation_price === 'number'
      ? p.trailing_activation_price
      : null;

  const unrealizedPnl = (isLong ? mark - entry : entry - mark) * p.quantity;
  const unrealizedPct = p.notional_usdt > 0 ? unrealizedPnl / p.notional_usdt : 0;
  const cushionPct = entry > 0 ? Math.abs(mark - stop) / entry : 0;
  const inProfit = unrealizedPnl >= 0;
  const pnlClass = inProfit ? 'text-accent' : 'text-destructive';

  // Two-zone "diverging" axis anchored at ENTRY so the fill shows PnL, not the
  // (much larger) distance to the stop. Left zone [0..C] = entry→stop (cushion
  // consumed); right zone [C..1] = entry→peak (profit runway, TP trigger marked).
  // The favorability transform (f) makes longs and shorts read the same way.
  const f = (price: number) => (isLong ? price : -price);
  const C = 0.42; // entry anchor position
  const fEntry = f(entry);
  const leftSpan = fEntry - f(stop) || 1;
  const rightMax = Math.max(
    f(peak),
    f(mark),
    tpTrigger !== null ? f(tpTrigger) : -Infinity
  );
  const rightSpan = rightMax - fEntry || 1;
  const posOf = (price: number) => {
    const fp = f(price);
    return fp <= fEntry
      ? C * Math.max(0, Math.min(1, (fp - f(stop)) / leftSpan))
      : C + (1 - C) * Math.max(0, Math.min(1, (fp - fEntry) / rightSpan));
  };
  const markPos = posOf(mark);
  const peakPos = posOf(peak);
  const tpPos = tpTrigger !== null ? posOf(tpTrigger) : null;

  // TP-trigger economics. armed = price has crossed the activation point.
  const tpProfitPct = tpTrigger !== null ? Math.abs(tpTrigger - entry) / entry : null;
  const armed = tpTrigger !== null && f(mark) >= f(tpTrigger);
  const overTriggerPct =
    tpTrigger !== null ? (f(mark) - f(tpTrigger)) / entry : null; // signed
  const peakOverTriggerPct =
    tpTrigger !== null ? Math.max(0, (f(peak) - f(tpTrigger)) / entry) : null;

  // Backend trailing state (peak persisted). When the trailing stop sits beyond
  // entry in the favorable direction it is LOCKING profit (lockedPct > 0); the
  // stop has stopped being a loss-cutter and become a profit-protector.
  const lockedPct = (f(stop) - fEntry) / entry;
  const trailLocking = trailing && lockedPct > 0;

  return (
    <div className="rounded-lg border border-border bg-secondary/40 px-3 py-2.5">
      {/* header: symbol/side · TP status · PnL */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <span className="text-sm font-bold text-foreground">{p.symbol}</span>
          <Badge
            className={cn(
              'h-5 px-1.5 text-[10px]',
              isLong
                ? 'border-accent/40 bg-accent/10 text-accent'
                : 'border-destructive/40 bg-destructive/10 text-destructive'
            )}
          >
            {p.side.toUpperCase()}
          </Badge>
        </div>

        <div className="flex items-center gap-3">
          {tpTrigger !== null && (
            <span
              className={cn(
                'rounded px-1.5 py-0.5 font-mono text-[10px] font-semibold tabular-nums',
                armed
                  ? 'bg-amber-400/15 text-amber-400'
                  : 'bg-muted/60 text-muted-foreground'
              )}
              title="TP trigger = trailing-stop activation"
            >
              {armed
                ? `${t('tpBeyond', { pct: formatPct(locale, (overTriggerPct ?? 0) * 100) })}${
                    peakOverTriggerPct && peakOverTriggerPct > 0
                      ? t('tpMax', { pct: formatPct(locale, peakOverTriggerPct * 100) })
                      : ''
                  }`
                : t('tpAt', { pct: formatPct(locale, (tpProfitPct ?? 0) * 100) })}
            </span>
          )}
          {trailing && (
            <span
              className={cn(
                'rounded px-1.5 py-0.5 font-mono text-[10px] font-semibold tabular-nums',
                trailLocking
                  ? 'bg-accent/15 text-accent'
                  : 'bg-amber-400/15 text-amber-400'
              )}
            >
              {trailLocking
                ? t('trailLocked', { pct: formatPct(locale, lockedPct * 100) })
                : t('trailArmed')}
            </span>
          )}
          <span
            className={cn(
              'w-16 text-right font-mono text-sm font-semibold tabular-nums',
              pnlClass
            )}
          >
            {formatPct(locale, unrealizedPct * 100)}
          </span>
          <span
            className={cn(
              'w-16 text-right font-mono text-sm tabular-nums',
              pnlClass
            )}
          >
            {formatUsd(locale, unrealizedPnl)}
          </span>
        </div>
      </div>

      {/* horizontal risk rail */}
      <div className="mt-2 flex items-center gap-2">
        <span
          className={cn(
            'w-8 shrink-0 text-right text-[9px] uppercase tracking-wide',
            trailLocking ? 'text-accent/80' : 'text-destructive/70'
          )}
        >
          {trailing ? t('trail') : t('stop')}
        </span>
        <div className="relative h-7 flex-1 overflow-hidden rounded-md border border-border bg-background/60">
          {/* faint zone tints: left = toward stop, right = toward profit/peak.
              When the trail is armed and locking profit the left zone is no longer
              danger — tint it accent to show the downside is protected. */}
          <div
            className={cn(
              'absolute inset-y-0 left-0',
              trailLocking ? 'bg-accent/10' : 'bg-destructive/5'
            )}
            style={{ width: `${C * 100}%` }}
          />
          <div
            className="absolute inset-y-0 right-0 bg-accent/5"
            style={{ left: `${C * 100}%` }}
          />
          {/* PnL fill: from the entry anchor to the current mark */}
          <div
            className={cn(
              'absolute inset-y-0 transition-all duration-500',
              inProfit ? 'bg-accent/30' : 'bg-destructive/30'
            )}
            style={{
              left: `${Math.min(C, markPos) * 100}%`,
              width: `${Math.abs(markPos - C) * 100}%`,
            }}
          />
          {/* over-trigger profit segment (TP armed): amber from trigger→mark */}
          {armed && tpPos !== null && markPos > tpPos && (
            <div
              className="absolute inset-y-0 bg-amber-400/35"
              style={{
                left: `${tpPos * 100}%`,
                width: `${(markPos - tpPos) * 100}%`,
              }}
            />
          )}
          {/* entry anchor (neutral baseline) */}
          <div
            className="absolute inset-y-0 w-px bg-muted-foreground/60"
            style={{ left: `${C * 100}%` }}
          />
          {/* TP trigger flag (amber) */}
          {tpPos !== null && (
            <div
              className="absolute inset-y-0 w-px bg-amber-400/80"
              style={{ left: `${tpPos * 100}%` }}
            >
              <span className="absolute -top-px left-1/2 -translate-x-1/2 border-x-[3px] border-t-[4px] border-x-transparent border-t-amber-400" />
            </div>
          )}
          {/* peak tick */}
          <div
            className="absolute inset-y-0 w-px bg-muted-foreground/40"
            style={{ left: `${peakPos * 100}%` }}
          />
          {/* mark dot */}
          <div
            className={cn(
              'absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-background',
              inProfit ? 'bg-accent' : 'bg-destructive'
            )}
            style={{ left: `${markPos * 100}%` }}
          />
        </div>
        <span className="w-8 shrink-0 text-[9px] uppercase tracking-wide text-muted-foreground/70">
          {t('peak')}
        </span>
      </div>

      {/* numeric strip */}
      <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 pl-10 text-[11px] text-muted-foreground">
        <Stat label={t('entry')} value={formatPrice(locale, entry)} dashed />
        <Stat
          label={t('mark')}
          value={formatPrice(locale, mark)}
          dotClass={inProfit ? 'bg-accent' : 'bg-destructive'}
        />
        {tpTrigger !== null && (
          <Stat label={t('tpArm')} value={formatPrice(locale, tpTrigger)} tpFlag />
        )}
        <Stat
          label={trailing ? t('trail') : t('stop')}
          value={formatPrice(locale, stop)}
          barClass={trailLocking ? 'bg-accent/70' : 'bg-destructive/70'}
        />
        <span className="ml-auto">
          {t('cushion')}{' '}
          <span className="font-mono font-semibold tabular-nums text-foreground">
            {formatNumber(locale, cushionPct * 100, 2)}%
          </span>
        </span>
      </div>
    </div>
  );
}

function Stat({
  label,
  value,
  dashed,
  dotClass,
  barClass,
  tpFlag,
}: {
  label: string;
  value: string;
  dashed?: boolean;
  dotClass?: string;
  barClass?: string;
  tpFlag?: boolean;
}) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="flex w-3 justify-center">
        {dotClass ? (
          <span className={cn('h-2 w-2 rounded-full', dotClass)} />
        ) : barClass ? (
          <span className={cn('h-0.5 w-3 rounded', barClass)} />
        ) : tpFlag ? (
          <span className="h-0 w-0 border-x-[3px] border-t-[4px] border-x-transparent border-t-amber-400" />
        ) : dashed ? (
          <span className="h-px w-3 border-t border-dashed border-muted-foreground/60" />
        ) : (
          <span className="h-px w-3 bg-border" />
        )}
      </span>
      <span className="uppercase tracking-wide text-muted-foreground">
        {label}
      </span>
      <span className="font-mono tabular-nums text-foreground">{value}</span>
    </span>
  );
}

/**
 * Open positions as horizontal risk-rail rows (one per token, full width).
 * Empty state stays informative: "flat — guards holding N setups".
 */
export function PositionGauge({
  positions,
  marketSignals = [],
  guardedSetups,
  className,
}: PositionGaugeProps) {
  const t = useT('positions');
  if (positions.length === 0) {
    return (
      <Card
        className={cn('border-0 bg-transparent shadow-none [&>*]:px-0', className)}
      >
        <CardHeader className="pb-2">
          <CardTitle className="text-lg text-foreground">{t('title')}</CardTitle>
        </CardHeader>
        <CardContent className="pt-0">
          <div className="py-8 text-center">
            <p className="font-medium text-foreground/80">
              {guardedSetups && guardedSetups > 0
                ? t('flatGuards', { n: guardedSetups })
                : t('flatNoPos')}
            </p>
            <p className="mt-1 text-sm text-muted-foreground">{t('flatNote')}</p>
          </div>
        </CardContent>
      </Card>
    );
  }

  const marketBySymbol = new Map(marketSignals.map(s => [s.symbol, s]));

  return (
    <Card className={cn('border-border bg-card', className)}>
      <CardHeader className="pb-2">
        <CardTitle className="text-lg text-foreground">
          {t('titleCount', { n: positions.length })}
        </CardTitle>
      </CardHeader>
      <CardContent className="pt-0">
        <div className="flex flex-col gap-2">
          {positions.map(p => (
            <PositionRow
              key={p.trade_id}
              p={p}
              signal={marketBySymbol.get(p.symbol)}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
