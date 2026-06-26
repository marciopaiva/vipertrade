'use client';

import { useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { cn } from '@/lib/utils';
import { useLocale, useT, formatSigned, formatRate, formatPct, formatNumber } from '@/lib/i18n';
import { Kpi } from '@/components/ui/Kpi';
import { SectionCard } from '@/components/ui/SectionCard';

type CloseReasonStat = {
  reason: string;
  trades: number;
  net_pnl: number;
  wins: number;
  avg_pnl_pct: number;
};
type FollowThroughStat = {
  armed: boolean;
  trades: number;
  net_pnl: number;
  wins: number;
  avg_pnl_pct: number;
};
type PeakCapture = {
  trailing_exits: number;
  avg_peak_pct: number;
  avg_realized_pct: number;
  pct_captured: number;
};
type SymbolStat = {
  symbol: string;
  realized_pnl: number;
  trades: number;
  wins: number;
  win_rate: number;
  avg_pnl_pct: number;
};
type TradeQuality = {
  window_days: number;
  closed_trades: number;
  net_pnl: number;
  win_rate: number;
  by_close_reason: CloseReasonStat[];
  follow_through: FollowThroughStat[];
  peak_capture: PeakCapture;
  worst_symbols: SymbolStat[];
};

const tone = (v: number) =>
  v > 0 ? 'text-accent' : v < 0 ? 'text-destructive' : 'text-muted-foreground';
// Close reasons are technical data values (thesis_invalidated, trailing_stop, …);
// prettify rather than translate.
const prettyReason = (s: string) =>
  s.replaceAll('_', ' ').replace(/\b\w/g, c => c.toUpperCase());

const WINDOWS = [7, 30, 90];

export default function LiveQualityTab() {
  const [days, setDays] = useState(7);
  const t = useT('live');
  const locale = useLocale();
  const { data, loading, error } = useDashboard<TradeQuality>(
    `/api/analysis/trade-quality?days=${days}`,
    { refreshInterval: 30000 }
  );

  const armed = data?.follow_through.find(f => f.armed);
  const notArmed = data?.follow_through.find(f => !f.armed);
  const cap = data?.peak_capture;

  return (
    <div className="space-y-5">
      <section className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-card p-5">
        <p className="max-w-2xl text-xs text-muted-foreground">{t('blurb')}</p>
        <div className="flex gap-1">
          {WINDOWS.map(w => (
            <button
              key={w}
              type="button"
              onClick={() => setDays(w)}
              className={cn(
                'rounded-md border px-2.5 py-1 text-xs font-medium',
                days === w
                  ? 'border-accent/40 bg-accent/10 text-accent'
                  : 'border-border text-muted-foreground hover:text-foreground'
              )}
            >
              {w}d
            </button>
          ))}
        </div>
      </section>

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && !data ? (
        <div className="h-72 animate-pulse rounded-xl border border-border bg-card" />
      ) : data && data.closed_trades === 0 ? (
        <div className="flex h-40 items-center justify-center rounded-xl border border-border bg-card text-sm text-muted-foreground">
          {t('empty', { days: data.window_days })}
        </div>
      ) : data ? (
        <>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Kpi label={t('kpiNet')} value={formatSigned(locale, data.net_pnl, 4)} tone={tone(data.net_pnl)} />
            <Kpi
              label={t('kpiWin')}
              value={formatRate(locale, data.win_rate)}
              tone={data.win_rate >= 0.5 ? 'text-accent' : 'text-foreground'}
            />
            <Kpi label={t('kpiClosed')} value={String(data.closed_trades)} />
            <Kpi
              label={t('kpiCapture')}
              value={cap ? `${formatNumber(locale, cap.pct_captured, 1)}%` : '—'}
              tone={cap && cap.pct_captured >= 50 ? 'text-accent' : 'text-foreground'}
            />
          </div>

          <SectionCard title={t('followTitle')}>
            <div className="grid gap-3 sm:grid-cols-2">
              {[armed, notArmed].map((f, i) =>
                f ? (
                  <div
                    key={i}
                    className={cn(
                      'rounded-lg border p-3',
                      f.armed ? 'border-accent/30 bg-accent/5' : 'border-destructive/30 bg-destructive/5'
                    )}
                  >
                    <div className="text-sm font-medium text-foreground">
                      {f.armed ? t('followArmed') : t('followNotArmed')}
                    </div>
                    <div className="mt-1 flex flex-wrap gap-x-4 text-xs text-muted-foreground">
                      <span>
                        net <span className={cn('font-mono', tone(f.net_pnl))}>{formatSigned(locale, f.net_pnl, 4)}</span>
                      </span>
                      <span>{t('statTrades', { n: f.trades })}</span>
                      <span>{t('statWins', { n: f.wins })}</span>
                      <span>
                        {t('statAvg')}{' '}
                        <span className={cn('font-mono', tone(f.avg_pnl_pct))}>
                          {formatPct(locale, f.avg_pnl_pct, 2)}
                        </span>
                      </span>
                    </div>
                  </div>
                ) : null
              )}
            </div>
          </SectionCard>

          {cap && cap.trailing_exits > 0 && (
            <SectionCard title={t('peakTitle', { n: cap.trailing_exits })}>
              <div className="flex flex-wrap items-center gap-x-6 gap-y-1 text-sm">
                <span className="text-muted-foreground">
                  {t('peakAvg')}{' '}
                  <span className="font-mono text-foreground">{formatPct(locale, cap.avg_peak_pct, 3)}</span>
                </span>
                <span className="text-muted-foreground">
                  {t('peakRealized')}{' '}
                  <span className={cn('font-mono', tone(cap.avg_realized_pct))}>
                    {formatPct(locale, cap.avg_realized_pct, 3)}
                  </span>
                </span>
                <span className="text-muted-foreground">
                  {t('peakLocked')}{' '}
                  <span
                    className={cn(
                      'font-mono font-semibold',
                      cap.pct_captured >= 50 ? 'text-accent' : 'text-foreground'
                    )}
                  >
                    {formatNumber(locale, cap.pct_captured, 1)}%
                  </span>
                </span>
              </div>
            </SectionCard>
          )}

          <SectionCard title={t('reasonsTitle')}>
            <div className="space-y-1.5">
              {data.by_close_reason.map(r => {
                const max = Math.max(1, ...data.by_close_reason.map(x => Math.abs(x.net_pnl)));
                const up = r.net_pnl >= 0;
                return (
                  <div key={r.reason} className="grid grid-cols-[150px_1fr_140px] items-center gap-3 text-xs">
                    <span className="truncate text-foreground">{prettyReason(r.reason)}</span>
                    <div className="h-2 rounded-full bg-background/60">
                      <div
                        className={cn('h-full rounded-full', up ? 'bg-accent/70' : 'bg-destructive/70')}
                        style={{ width: `${(Math.abs(r.net_pnl) / max) * 100}%` }}
                      />
                    </div>
                    <span className="text-right font-mono tabular-nums text-muted-foreground">
                      <span className={tone(r.net_pnl)}>{formatSigned(locale, r.net_pnl, 4)}</span> · {r.trades}t · {r.wins}w
                    </span>
                  </div>
                );
              })}
            </div>
          </SectionCard>

          <SectionCard title={t('bySymbolTitle')}>
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-left text-[10px] uppercase tracking-wider text-muted-foreground">
                    <th className="pb-2 pr-3 font-medium">{t('colSymbol')}</th>
                    <th className="pb-2 pr-3 text-right font-medium">{t('colNet')}</th>
                    <th className="pb-2 pr-3 text-right font-medium">{t('colTrades')}</th>
                    <th className="pb-2 text-right font-medium">{t('colWin')}</th>
                  </tr>
                </thead>
                <tbody className="font-mono tabular-nums">
                  {data.worst_symbols.map(s => (
                    <tr key={s.symbol} className="border-t border-border/50">
                      <td className="py-1.5 pr-3 text-foreground">{s.symbol}</td>
                      <td className={cn('py-1.5 pr-3 text-right', tone(s.realized_pnl))}>
                        {formatSigned(locale, s.realized_pnl, 4)}
                      </td>
                      <td className="py-1.5 pr-3 text-right text-muted-foreground">{s.trades}</td>
                      <td className="py-1.5 text-right text-muted-foreground">
                        {formatNumber(locale, s.win_rate * 100, 0)}%
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </SectionCard>
        </>
      ) : null}
    </div>
  );
}
