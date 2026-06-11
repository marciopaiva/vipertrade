'use client';

import { useMemo, useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { CloseReasonAttribution } from '@/components/trades/CloseReasonAttribution';
import { TradesTable } from '@/components/trades/TradesTable';
import { cn } from '@/lib/utils';
import type { Trade } from '@/types/trading';

type SideFilter = 'all' | 'long' | 'short';
type StatusFilter = 'all' | 'closed' | 'open';

// Module scope: these read the wall clock (impure), so they live outside the
// component body — the react-hooks purity rule forbids Date.now() during render.
const todayISO = () => new Date().toISOString().slice(0, 10);
const daysAgoISO = (days: number) =>
  new Date(Date.now() - days * 864e5).toISOString().slice(0, 10);

/**
 * Keep a trade if its timestamp (closed_at, else opened_at) falls within the
 * inclusive [from, to] day range. Empty bound = open-ended. Client-side, on the
 * loaded window (API caps at 200 trades; a server-side `since`/`until` param is
 * the follow-up once history outgrows that).
 */
function inDateRange(t: Trade, from: string, to: string): boolean {
  const ts = Date.parse(t.closed_at || t.opened_at);
  if (Number.isNaN(ts)) return true;
  if (from) {
    const f = Date.parse(`${from}T00:00:00`);
    if (!Number.isNaN(f) && ts < f) return false;
  }
  if (to) {
    const e = Date.parse(`${to}T23:59:59.999`);
    if (!Number.isNaN(e) && ts > e) return false;
  }
  return true;
}

function DateField({
  label,
  value,
  onChange,
  max,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  max?: string;
}) {
  return (
    <label className="flex items-center gap-2 text-xs text-muted-foreground">
      <span className="uppercase tracking-wide">{label}</span>
      <input
        type="date"
        value={value}
        max={max}
        onChange={e => onChange(e.target.value)}
        className="rounded-md border border-border bg-card px-2 py-1 text-foreground outline-none transition-colors [color-scheme:dark] focus:border-primary/50"
      />
    </label>
  );
}

function Select({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
}) {
  return (
    <label className="flex items-center gap-2 text-xs text-muted-foreground">
      <span className="uppercase tracking-wide">{label}</span>
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        className="rounded-md border border-border bg-card px-2 py-1 text-foreground outline-none transition-colors focus:border-primary/50"
      >
        {options.map(o => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

export default function TradesPage() {
  const { data, loading, error } = useDashboard<{ items: Trade[] }>(
    '/api/v1/trades?limit=200',
    { refreshInterval: 10000 }
  );
  const trades = useMemo(() => data?.items ?? [], [data]);

  const [symbol, setSymbol] = useState('all');
  const [side, setSide] = useState<SideFilter>('all');
  const [status, setStatus] = useState<StatusFilter>('all');
  const [reason, setReason] = useState<string | null>(null);
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const today = todayISO();
  const setRange = (days: number) => {
    setDateTo(today);
    setDateFrom(days === 0 ? today : daysAgoISO(days));
  };

  const symbols = useMemo(
    () => [...new Set(trades.map(t => t.symbol))].sort(),
    [trades]
  );

  // Universe for the attribution cards: symbol + side, but NOT reason (so the
  // cards always show every reason; clicking one sets the reason filter).
  const base = useMemo(
    () =>
      trades.filter(t => {
        if (symbol !== 'all' && t.symbol !== symbol) return false;
        if (side !== 'all' && t.side.toLowerCase() !== side) return false;
        if (!inDateRange(t, dateFrom, dateTo)) return false;
        return true;
      }),
    [trades, symbol, side, dateFrom, dateTo]
  );

  const tableRows = useMemo(
    () =>
      base.filter(t => {
        if (status === 'closed' && t.status !== 'closed') return false;
        if (status === 'open' && t.status === 'closed') return false;
        if (reason && (t.close_reason || 'unknown') !== reason) return false;
        return true;
      }),
    [base, status, reason]
  );

  const closed = useMemo(
    () => tableRows.filter(t => t.status === 'closed'),
    [tableRows]
  );
  const netPnl = closed.reduce((sum, t) => sum + (t.pnl ?? 0), 0);
  const wins = closed.filter(t => (t.pnl ?? 0) >= 0).length;
  const winRate = closed.length ? (wins / closed.length) * 100 : 0;

  return (
    <div className="space-y-5">
      {/* header */}
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">
            Trades
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            The ledger with PnL attribution by close-reason — where the edge and
            the bleed come from.
          </p>
        </div>
        <div className="flex items-center gap-5 font-mono text-sm tabular-nums">
          <span className="text-muted-foreground">
            net{' '}
            <span className={netPnl >= 0 ? 'text-accent' : 'text-destructive'}>
              {netPnl >= 0 ? '+' : '−'}${Math.abs(netPnl).toFixed(2)}
            </span>
          </span>
          <span className="text-muted-foreground">
            <span className="text-foreground">{closed.length}</span> closed
          </span>
          <span className="text-muted-foreground">
            <span className="text-foreground">{winRate.toFixed(0)}%</span> win
          </span>
        </div>
      </div>

      {/* filters */}
      <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
        <Select
          label="Symbol"
          value={symbol}
          onChange={setSymbol}
          options={[
            { value: 'all', label: 'All' },
            ...symbols.map(s => ({ value: s, label: s })),
          ]}
        />
        <Select
          label="Side"
          value={side}
          onChange={v => setSide(v as SideFilter)}
          options={[
            { value: 'all', label: 'All' },
            { value: 'long', label: 'Long' },
            { value: 'short', label: 'Short' },
          ]}
        />
        <Select
          label="Status"
          value={status}
          onChange={v => setStatus(v as StatusFilter)}
          options={[
            { value: 'all', label: 'All' },
            { value: 'closed', label: 'Closed' },
            { value: 'open', label: 'Open' },
          ]}
        />
        <DateField
          label="From"
          value={dateFrom}
          onChange={setDateFrom}
          max={dateTo || today}
        />
        <DateField label="To" value={dateTo} onChange={setDateTo} max={today} />
        <div className="flex items-center gap-1">
          {[
            { label: 'Today', days: 0 },
            { label: '7d', days: 7 },
            { label: '30d', days: 30 },
          ].map(p => (
            <button
              key={p.label}
              type="button"
              onClick={() => setRange(p.days)}
              className="rounded-md border border-border bg-card px-2 py-1 text-xs text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
            >
              {p.label}
            </button>
          ))}
          {(dateFrom || dateTo) && (
            <button
              type="button"
              onClick={() => {
                setDateFrom('');
                setDateTo('');
              }}
              className="rounded-md border border-primary/40 bg-primary/10 px-2 py-1 text-xs text-primary transition-colors hover:bg-primary/15"
            >
              clear dates ✕
            </button>
          )}
        </div>
        {reason && (
          <button
            type="button"
            onClick={() => setReason(null)}
            className="inline-flex items-center gap-1 rounded-md border border-primary/40 bg-primary/10 px-2 py-1 text-xs text-primary transition-colors hover:bg-primary/15"
          >
            reason: {reason.replaceAll('_', ' ')} ✕
          </button>
        )}
      </div>

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && trades.length === 0 ? (
        <div className="space-y-3">
          <div className="h-24 animate-pulse rounded-xl border border-border bg-card" />
          <div className="h-96 animate-pulse rounded-xl border border-border bg-card" />
        </div>
      ) : (
        <>
          <CloseReasonAttribution
            trades={base}
            activeReason={reason}
            onSelectReason={setReason}
          />
          <TradesTable trades={tableRows} />
        </>
      )}
    </div>
  );
}
