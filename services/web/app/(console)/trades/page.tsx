'use client';

import { useMemo, useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { useT, useLocale, formatUsd, formatNumber } from '@/lib/i18n';
import { CloseReasonAttribution } from '@/components/trades/CloseReasonAttribution';
import { TradesTable } from '@/components/trades/TradesTable';
import { reasonLabel } from '@/components/trades/reasonLabel';
import { cn } from '@/lib/utils';
import { SelectField, DateField, FilterChip } from '@/components/ui/Field';
import type { Trade } from '@/types/trading';
import { PageHeader } from '@/components/ui/PageHeader';

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

export default function TradesPage() {
  const t = useT('trades');
  const locale = useLocale();
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
      <PageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        right={
          <div className="flex items-center gap-5 font-mono text-sm tabular-nums">
            <span className="text-muted-foreground">
              {t('net')}{' '}
              <span
                className={netPnl >= 0 ? 'text-accent' : 'text-destructive'}
              >
                {formatUsd(locale, netPnl)}
              </span>
            </span>
            <span className="text-muted-foreground">
              <span className="text-foreground">{closed.length}</span>{' '}
              {t('closed')}
            </span>
            <span className="text-muted-foreground">
              <span className="text-foreground">
                {formatNumber(locale, winRate, 0)}%
              </span>{' '}
              {t('win')}
            </span>
          </div>
        }
      />

      {/* filters */}
      <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
        <SelectField
          label={t('fSymbol')}
          value={symbol}
          onChange={setSymbol}
          options={[
            { value: 'all', label: t('all') },
            ...symbols.map(s => ({ value: s, label: s })),
          ]}
        />
        <SelectField
          label={t('fSide')}
          value={side}
          onChange={v => setSide(v as SideFilter)}
          options={[
            { value: 'all', label: t('all') },
            { value: 'long', label: t('long') },
            { value: 'short', label: t('short') },
          ]}
        />
        <SelectField
          label={t('fStatus')}
          value={status}
          onChange={v => setStatus(v as StatusFilter)}
          options={[
            { value: 'all', label: t('all') },
            { value: 'closed', label: t('statusClosed') },
            { value: 'open', label: t('statusOpen') },
          ]}
        />
        <DateField
          label={t('fFrom')}
          value={dateFrom}
          onChange={setDateFrom}
          max={dateTo || today}
        />
        <DateField
          label={t('fTo')}
          value={dateTo}
          onChange={setDateTo}
          max={today}
        />
        <div className="flex items-center gap-1">
          {[
            { label: t('rangeToday'), days: 0 },
            { label: t('range7d'), days: 7 },
            { label: t('range30d'), days: 30 },
          ].map(p => (
            <FilterChip key={p.days} onClick={() => setRange(p.days)}>
              {p.label}
            </FilterChip>
          ))}
          {(dateFrom || dateTo) && (
            <FilterChip
              active
              onClick={() => {
                setDateFrom('');
                setDateTo('');
              }}
            >
              {t('clearDates')}
            </FilterChip>
          )}
        </div>
        {reason && (
          <FilterChip active onClick={() => setReason(null)}>
            {t('reasonChip', { reason: reasonLabel(t, reason) })}
          </FilterChip>
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
