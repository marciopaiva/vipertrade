'use client';

import { useEffect, useMemo, useState } from 'react';
import { cn } from '@/lib/utils';
import { Sparkline } from './Sparkline';

interface Trade {
  trade_id: string;
  status: string;
  pnl?: number;
  opened_at: string;
  closed_at?: string;
}

interface KpiStripProps {
  equity?: number;
  /** Realized PnL over the last 24h (used as the equity delta + curve scale). */
  pnl24h?: number;
  /** Win rate over the last 24h, already in percent (0–100). */
  winRate24h?: number;
  openCount: number;
  todayCount: number;
  /** Closed trades (any window) — the 24h slice drives the equity curve. */
  trades: Trade[];
}

function usd(value?: number) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '—';
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 2,
  }).format(value);
}

function signedUsd(value?: number) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '—';
  const sign = value > 0 ? '+' : value < 0 ? '−' : '';
  return `${sign}${usd(Math.abs(value))}`;
}

function Stat({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        {label}
      </span>
      <span className="font-mono text-lg font-semibold tabular-nums text-foreground">
        {children}
      </span>
    </div>
  );
}

/**
 * The Console at-a-glance strip: equity + 24h realized PnL (number and curve),
 * win rate, open positions, trades today. Mono tabular so digits don't jitter
 * on live refresh. The "equity curve" is derived from cumulative realized PnL of
 * the last 24h of closed trades — there is no equity time-series in the API, so
 * this is an honest realized-PnL curve, not a mark-to-market equity line.
 */
export function KpiStrip({
  equity,
  pnl24h,
  winRate24h,
  openCount,
  todayCount,
  trades,
}: KpiStripProps) {
  // Hold "now" in state (Date.now() is impure during render) and slide the 24h
  // window forward periodically so a long-open page stays honest.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 60_000);
    return () => clearInterval(id);
  }, []);

  const series = useMemo(() => {
    const since = now - 24 * 60 * 60 * 1000;
    const closed = trades
      .filter(t => {
        if (t.status !== 'closed') return false;
        const ts = Date.parse(t.closed_at || t.opened_at);
        return Number.isFinite(ts) && ts >= since;
      })
      .sort(
        (a, b) =>
          Date.parse(a.closed_at || a.opened_at) -
          Date.parse(b.closed_at || b.opened_at)
      );

    let running = 0;
    const points = [0];
    for (const t of closed) {
      running += t.pnl ?? 0;
      points.push(running);
    }
    return points;
  }, [trades, now]);

  const up = (pnl24h ?? 0) >= 0;

  return (
    <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-4 rounded-xl border border-border bg-card px-5 py-4">
      {/* equity + 24h delta + curve */}
      <div className="flex items-end gap-5">
        <div className="flex flex-col gap-0.5">
          <span className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
            Equity
          </span>
          <div className="flex items-baseline gap-2.5">
            <span className="font-mono text-3xl font-bold tabular-nums tracking-tight text-foreground">
              {usd(equity)}
            </span>
            <span
              className={cn(
                'font-mono text-sm font-semibold tabular-nums',
                up ? 'text-accent' : 'text-destructive'
              )}
            >
              {up ? '▴' : '▾'} {signedUsd(pnl24h)}
              <span className="ml-1 text-[11px] font-normal text-muted-foreground">
                24h
              </span>
            </span>
          </div>
        </div>
        <Sparkline
          values={series}
          colorClassName={up ? 'text-accent' : 'text-destructive'}
          className="mb-0.5"
        />
      </div>

      {/* secondary stats */}
      <div className="flex items-center gap-x-8">
        <Stat label="Win rate">
          {typeof winRate24h === 'number' ? `${winRate24h.toFixed(0)}%` : '—'}
        </Stat>
        <Stat label="Open">{openCount}</Stat>
        <Stat label="Today">
          {todayCount}
          <span className="ml-1 text-xs font-normal text-muted-foreground">
            trades
          </span>
        </Stat>
      </div>
    </div>
  );
}
