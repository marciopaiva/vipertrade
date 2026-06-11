'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

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
  break_even_price?: number | null;
}

interface PositionGaugeProps {
  positions: Position[];
  marketSignals?: MarketSignal[];
  guardedSetups?: number;
  className?: string;
}

const fmtPrice = (v: number) => (v >= 100 ? v.toFixed(2) : v.toFixed(4));
const fmtPct = (v: number) => `${v >= 0 ? '+' : ''}${(v * 100).toFixed(2)}%`;
const fmtUsd = (v: number) => `${v >= 0 ? '+' : ''}$${v.toFixed(2)}`;

/**
 * One open position as a vertical "risk rail": the current mark between its
 * active stop (bottom) and its peak (top), filled by how favorable it is and
 * colored by unrealized PnL. There is no fixed take-profit (trailing strategy),
 * so the gauge is oriented by *favorability* — higher = more profit — which
 * makes longs and shorts read the same way (up = good, down = toward the stop).
 */
function PositionThermometer({
  p,
  signal,
}: {
  p: Position;
  signal?: MarketSignal;
}) {
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

  const unrealizedPnl = (isLong ? mark - entry : entry - mark) * p.quantity;
  const unrealizedPct = p.notional_usdt > 0 ? unrealizedPnl / p.notional_usdt : 0;
  const cushionPct = entry > 0 ? Math.abs(mark - stop) / entry : 0;
  const inProfit = unrealizedPnl >= 0;
  const pnlClass = inProfit ? 'text-accent' : 'text-destructive';

  // Favorability axis: higher = more profit (flip sign for shorts).
  const f = (price: number) => (isLong ? price : -price);
  const lo = f(stop);
  const hi = Math.max(f(peak), f(mark), f(entry));
  const span = hi - lo || 1;
  const frac = (price: number) =>
    Math.max(0, Math.min(1, (f(price) - lo) / span));
  const markFrac = frac(mark);
  const entryFrac = frac(entry);

  return (
    <div className="rounded-lg border border-border bg-secondary/40 p-3">
      <div className="mb-2 flex items-center justify-between gap-2">
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
        <span
          className={cn(
            'font-mono text-sm font-semibold tabular-nums',
            inProfit ? 'text-accent' : 'text-destructive'
          )}
        >
          {fmtPct(unrealizedPct)}
        </span>
      </div>

      <div className="flex gap-3">
        {/* the rail */}
        <div className="relative h-36 w-12 overflow-hidden rounded-md border border-border bg-background/60">
          {/* fill from the stop (bottom) up to the current mark */}
          <div
            className={cn(
              'absolute inset-x-0 bottom-0 transition-[height] duration-500',
              inProfit ? 'bg-accent/25' : 'bg-destructive/25'
            )}
            style={{ height: `${markFrac * 100}%` }}
          />
          {/* mark line + dot at the top of the fill */}
          <div
            className={cn(
              'absolute inset-x-0 h-px',
              inProfit ? 'bg-accent' : 'bg-destructive'
            )}
            style={{ bottom: `calc(${markFrac * 100}% - 0.5px)` }}
          >
            <span
              className={cn(
                'absolute -right-0.5 -top-1 h-2 w-2 rounded-full',
                inProfit ? 'bg-accent' : 'bg-destructive'
              )}
            />
          </div>
          {/* entry reference (dashed) */}
          <div
            className="absolute inset-x-0 border-t border-dashed border-muted-foreground/50"
            style={{ bottom: `${entryFrac * 100}%` }}
          />
          {/* stop baseline */}
          <div className="absolute inset-x-0 bottom-0 border-t-2 border-destructive/60" />
        </div>

        {/* legend */}
        <div className="flex flex-1 flex-col justify-between py-0.5 text-[11px]">
          <Legend label="peak" value={fmtPrice(peak)} muted />
          <Legend
            label="mark"
            value={fmtPrice(mark)}
            dotClass={inProfit ? 'bg-accent' : 'bg-destructive'}
          />
          <Legend label="entry" value={fmtPrice(entry)} dashed />
          <Legend
            label="stop"
            value={fmtPrice(stop)}
            barClass="bg-destructive/70"
            suffix={trailing ? 'trail' : 'SL'}
          />
        </div>
      </div>

      <div className="mt-2 flex items-center justify-between border-t border-border pt-2 text-[11px] text-muted-foreground">
        <span>
          cushion to stop{' '}
          <span className="font-mono font-semibold tabular-nums text-foreground">
            {(cushionPct * 100).toFixed(2)}%
          </span>
        </span>
        <span className={cn('font-mono tabular-nums', pnlClass)}>
          {fmtUsd(unrealizedPnl)}
        </span>
      </div>
    </div>
  );
}

function Legend({
  label,
  value,
  muted,
  dashed,
  dotClass,
  barClass,
  suffix,
}: {
  label: string;
  value: string;
  muted?: boolean;
  dashed?: boolean;
  dotClass?: string;
  barClass?: string;
  suffix?: string;
}) {
  return (
    <div className="flex items-center gap-1.5">
      <span className="flex w-3 justify-center">
        {dotClass ? (
          <span className={cn('h-2 w-2 rounded-full', dotClass)} />
        ) : barClass ? (
          <span className={cn('h-0.5 w-3 rounded', barClass)} />
        ) : dashed ? (
          <span className="h-px w-3 border-t border-dashed border-muted-foreground/60" />
        ) : (
          <span className="h-px w-3 bg-border" />
        )}
      </span>
      <span
        className={cn(
          'w-9 uppercase tracking-wide',
          muted ? 'text-muted-foreground/70' : 'text-muted-foreground'
        )}
      >
        {label}
      </span>
      <span className="font-mono tabular-nums text-foreground">{value}</span>
      {suffix && (
        <span className="text-[9px] uppercase text-muted-foreground/70">
          {suffix}
        </span>
      )}
    </div>
  );
}

/**
 * Open positions as vertical risk-rail cards (replaces the table on /console).
 * Empty state stays informative: "flat — guards holding N setups".
 */
export function PositionGauge({
  positions,
  marketSignals = [],
  guardedSetups,
  className,
}: PositionGaugeProps) {
  if (positions.length === 0) {
    return (
      <Card
        className={cn('border-0 bg-transparent shadow-none [&>*]:px-0', className)}
      >
        <CardHeader className="pb-2">
          <CardTitle className="text-lg text-foreground">
            Open Positions
          </CardTitle>
        </CardHeader>
        <CardContent className="pt-0">
          <div className="py-8 text-center">
            <p className="font-medium text-foreground/80">
              {guardedSetups && guardedSetups > 0
                ? `Flat — guards holding ${guardedSetups} setup${guardedSetups === 1 ? '' : 's'}`
                : 'Flat — no open positions'}
            </p>
            <p className="mt-1 text-sm text-muted-foreground">
              The strategy is monitoring the market; entries open when exchange
              consensus and the entry guards align.
            </p>
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
          Open Positions ({positions.length})
        </CardTitle>
      </CardHeader>
      <CardContent className="pt-0">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {positions.map(p => (
            <PositionThermometer
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
