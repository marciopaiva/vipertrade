'use client';

import { useMemo, useState } from 'react';
import { cn } from '@/lib/utils';
import {
  useT,
  useLocale,
  formatNumber,
  formatUsd,
  formatPct,
  type Locale,
} from '@/lib/i18n';
import { reasonLabel } from './reasonLabel';
import type { Trade } from '@/types/trading';

type T = ReturnType<typeof useT<'trades'>>;
type TKey = Parameters<T>[0];

type SortKey = 'closed_at' | 'symbol' | 'pnl' | 'duration_seconds';
type SortDir = 'asc' | 'desc';

const PAGE_SIZE = 25;

function fmtPrice(locale: Locale, v?: number | null) {
  return typeof v === 'number' ? `$${formatNumber(locale, v, 6)}` : '—';
}

function fmtWhen(locale: Locale, iso?: string | null) {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '—';
  return {
    date: d.toLocaleDateString(locale, { month: 'short', day: 'numeric' }),
    time: d.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' }),
  };
}

function fmtDuration(s?: number) {
  if (!s || s <= 0) return '—';
  if (s < 3600) return `${Math.max(1, Math.round(s / 60))}m`;
  const h = Math.floor(s / 3600);
  const m = Math.round((s % 3600) / 60);
  return m ? `${h}h ${m}m` : `${h}h`;
}

function pnlPct(t: Trade) {
  if (typeof t.pnl_pct === 'number') return t.pnl_pct;
  const notional = (t.entry_price || 0) * (t.quantity || 0);
  if (typeof t.pnl === 'number' && notional > 0) return t.pnl / notional;
  return null;
}

const COLUMNS: {
  key: SortKey | null;
  label: TKey;
  className: string;
  align?: 'right';
}[] = [
  { key: 'symbol', label: 'colAsset', className: 'w-[140px]' },
  { key: null, label: 'colSide', className: 'w-[64px]' },
  { key: 'pnl', label: 'colPnl', className: 'w-[110px]', align: 'right' },
  { key: null, label: 'colEntry', className: 'w-[120px]' },
  { key: null, label: 'colExit', className: 'w-[120px]' },
  { key: null, label: 'colReason', className: 'flex-1 min-w-[120px]' },
  { key: 'closed_at', label: 'colClosed', className: 'w-[110px]' },
  { key: 'duration_seconds', label: 'colHeld', className: 'w-[80px]' },
];

export function TradesTable({ trades }: { trades: Trade[] }) {
  const t = useT('trades');
  const locale = useLocale();
  const [sortKey, setSortKey] = useState<SortKey>('closed_at');
  const [sortDir, setSortDir] = useState<SortDir>('desc');
  const [page, setPage] = useState(0);

  const sorted = useMemo(() => {
    const dir = sortDir === 'asc' ? 1 : -1;
    return [...trades].sort((a, b) => {
      switch (sortKey) {
        case 'symbol':
          return a.symbol.localeCompare(b.symbol) * dir;
        case 'pnl':
          return ((a.pnl ?? 0) - (b.pnl ?? 0)) * dir;
        case 'duration_seconds':
          return ((a.duration_seconds ?? 0) - (b.duration_seconds ?? 0)) * dir;
        case 'closed_at':
        default:
          return (
            (Date.parse(a.closed_at || a.opened_at) -
              Date.parse(b.closed_at || b.opened_at)) *
            dir
          );
      }
    });
  }, [trades, sortKey, sortDir]);

  const totalPages = Math.max(1, Math.ceil(sorted.length / PAGE_SIZE));
  const safePage = Math.min(page, totalPages - 1);
  const pageRows = sorted.slice(
    safePage * PAGE_SIZE,
    safePage * PAGE_SIZE + PAGE_SIZE
  );

  function toggleSort(key: SortKey) {
    if (key === sortKey) {
      setSortDir(d => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortKey(key);
      setSortDir(key === 'symbol' ? 'asc' : 'desc');
    }
    setPage(0);
  }

  if (trades.length === 0) {
    return (
      <div className="rounded-xl border border-border bg-card px-3 py-12 text-center text-sm text-muted-foreground">
        {t('emptyFiltered')}
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="overflow-hidden rounded-xl border border-border bg-card">
        {/* header */}
        <div className="hidden gap-4 border-b border-border px-4 py-2.5 text-[11px] uppercase tracking-[0.16em] text-muted-foreground lg:flex">
          {COLUMNS.map(col => (
            <div
              key={col.label}
              className={cn(
                col.className,
                col.align === 'right' && 'text-right'
              )}
            >
              {col.key ? (
                <button
                  type="button"
                  onClick={() => toggleSort(col.key as SortKey)}
                  className={cn(
                    'inline-flex items-center gap-1 transition-colors hover:text-foreground',
                    sortKey === col.key && 'text-foreground'
                  )}
                >
                  {t(col.label)}
                  <span className="text-[9px]">
                    {sortKey === col.key
                      ? sortDir === 'asc'
                        ? '▲'
                        : '▼'
                      : '↕'}
                  </span>
                </button>
              ) : (
                t(col.label)
              )}
            </div>
          ))}
        </div>

        {/* rows */}
        <div>
          {pageRows.map(row => {
            const pnl = row.pnl ?? 0;
            const win = pnl >= 0;
            const isLong = row.side.toLowerCase() === 'long';
            const pct = pnlPct(row);
            const closed = fmtWhen(locale, row.closed_at);
            const open = row.status !== 'closed';
            return (
              <div
                key={row.trade_id}
                className="flex flex-col gap-2 border-b border-border/50 px-4 py-2.5 text-sm last:border-b-0 lg:flex-row lg:items-center lg:gap-4"
              >
                <div className="w-[140px] font-semibold text-foreground">
                  {row.symbol}
                </div>
                <div
                  className={cn(
                    'w-[64px] text-xs font-semibold uppercase',
                    isLong ? 'text-accent' : 'text-destructive'
                  )}
                >
                  {isLong ? t('long') : t('short')}
                </div>
                <div className="w-[110px] text-right">
                  {open ? (
                    <span className="text-muted-foreground">{t('rowOpen')}</span>
                  ) : (
                    <>
                      <div
                        className={cn(
                          'font-mono font-semibold tabular-nums',
                          win ? 'text-accent' : 'text-destructive'
                        )}
                      >
                        {formatUsd(locale, pnl)}
                      </div>
                      {pct !== null && (
                        <div
                          className={cn(
                            'font-mono text-xs tabular-nums',
                            win ? 'text-accent/80' : 'text-destructive/80'
                          )}
                        >
                          {formatPct(locale, pct * 100)}
                        </div>
                      )}
                    </>
                  )}
                </div>
                <div className="w-[120px] font-mono text-xs tabular-nums text-muted-foreground">
                  {fmtPrice(locale, row.entry_price)}
                </div>
                <div className="w-[120px] font-mono text-xs tabular-nums text-muted-foreground">
                  {fmtPrice(locale, row.exit_price)}
                </div>
                <div className="flex-1 min-w-[120px] truncate text-xs text-foreground/90">
                  {open ? (
                    <span className="text-muted-foreground">—</span>
                  ) : (
                    reasonLabel(t, row.close_reason)
                  )}
                </div>
                <div className="w-[110px] font-mono text-xs tabular-nums text-muted-foreground">
                  {typeof closed === 'object' ? (
                    <>
                      <span className="text-foreground/80">{closed.date}</span>{' '}
                      {closed.time}
                    </>
                  ) : (
                    closed
                  )}
                </div>
                <div className="w-[80px] font-mono text-xs tabular-nums text-muted-foreground">
                  {fmtDuration(row.duration_seconds)}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span className="font-mono tabular-nums">
            {t('pageInfo', {
              n: sorted.length,
              page: safePage + 1,
              total: totalPages,
            })}
          </span>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => setPage(p => Math.max(0, p - 1))}
              disabled={safePage === 0}
              className="rounded-md border border-border px-2.5 py-1 transition-colors hover:border-primary/40 disabled:opacity-40"
            >
              {t('prev')}
            </button>
            <button
              type="button"
              onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
              disabled={safePage >= totalPages - 1}
              className="rounded-md border border-border px-2.5 py-1 transition-colors hover:border-primary/40 disabled:opacity-40"
            >
              {t('next')}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
