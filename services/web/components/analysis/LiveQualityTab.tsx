'use client';

import { useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { cn } from '@/lib/utils';

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

const signed = (v: number, d = 4) =>
  `${v >= 0 ? '+' : '−'}${Math.abs(v).toFixed(d)}`;
const tone = (v: number) =>
  v > 0 ? 'text-accent' : v < 0 ? 'text-destructive' : 'text-muted-foreground';
const title = (s: string) =>
  s.replaceAll('_', ' ').replace(/\b\w/g, c => c.toUpperCase());

function Kpi({ label, value, tone: t }: { label: string; value: string; tone?: string }) {
  return (
    <div className="rounded-lg border border-border bg-card p-3">
      <div className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">{label}</div>
      <div className={cn('mt-1 font-mono text-lg tabular-nums', t ?? 'text-foreground')}>{value}</div>
    </div>
  );
}

const WINDOWS = [7, 30, 90];

export default function LiveQualityTab() {
  const [days, setDays] = useState(7);
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
        <p className="max-w-2xl text-xs text-muted-foreground">
          Qualidade de operação sobre trades <strong>realizados</strong> (fechados, paper) — dados
          reais, não backtest. Atualiza a cada 30s.
        </p>
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
          Sem trades fechados nos últimos {data.window_days}d.
        </div>
      ) : data ? (
        <>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Kpi label="Net PnL realizado" value={signed(data.net_pnl)} tone={tone(data.net_pnl)} />
            <Kpi
              label="Win rate"
              value={`${(data.win_rate * 100).toFixed(1)}%`}
              tone={data.win_rate >= 0.5 ? 'text-accent' : 'text-foreground'}
            />
            <Kpi label="Trades fechados" value={String(data.closed_trades)} />
            <Kpi
              label="Captura do pico"
              value={cap ? `${cap.pct_captured.toFixed(1)}%` : '—'}
              tone={cap && cap.pct_captured >= 50 ? 'text-accent' : 'text-foreground'}
            />
          </div>

          {/* Follow-through: the headline insight — never-armed entries are the losses */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
              Follow-through de entrada · armou o trailing vs morreu flat
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              {[armed, notArmed].map((f, i) =>
                f ? (
                  <div
                    key={i}
                    className={cn(
                      'rounded-lg border p-3',
                      f.armed
                        ? 'border-accent/30 bg-accent/5'
                        : 'border-destructive/30 bg-destructive/5'
                    )}
                  >
                    <div className="text-sm font-medium text-foreground">
                      {f.armed ? 'Armou o trailing (andou ≥ +0,1%)' : 'Nunca armou (entrou e não andou)'}
                    </div>
                    <div className="mt-1 flex flex-wrap gap-x-4 text-xs text-muted-foreground">
                      <span>
                        net <span className={cn('font-mono', tone(f.net_pnl))}>{signed(f.net_pnl)}</span>
                      </span>
                      <span>{f.trades} trades</span>
                      <span>{f.wins} wins</span>
                      <span>
                        avg{' '}
                        <span className={cn('font-mono', tone(f.avg_pnl_pct))}>
                          {signed(f.avg_pnl_pct, 2)}%
                        </span>
                      </span>
                    </div>
                  </div>
                ) : null
              )}
            </div>
          </section>

          {/* Trailing peak-capture */}
          {cap && cap.trailing_exits > 0 && (
            <section className="rounded-xl border border-border bg-card p-5">
              <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Captura do pico · trailing exits ({cap.trailing_exits})
              </div>
              <div className="flex flex-wrap items-center gap-x-6 gap-y-1 text-sm">
                <span className="text-muted-foreground">
                  pico médio{' '}
                  <span className="font-mono text-foreground">{signed(cap.avg_peak_pct, 3)}%</span>
                </span>
                <span className="text-muted-foreground">
                  realizado{' '}
                  <span className={cn('font-mono', tone(cap.avg_realized_pct))}>
                    {signed(cap.avg_realized_pct, 3)}%
                  </span>
                </span>
                <span className="text-muted-foreground">
                  travado{' '}
                  <span
                    className={cn(
                      'font-mono font-semibold',
                      cap.pct_captured >= 50 ? 'text-accent' : 'text-foreground'
                    )}
                  >
                    {cap.pct_captured.toFixed(1)}%
                  </span>
                </span>
              </div>
            </section>
          )}

          {/* Close-reason attribution */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
              Atribuição por motivo de saída
            </div>
            <div className="space-y-1.5">
              {data.by_close_reason.map(r => {
                const max = Math.max(1, ...data.by_close_reason.map(x => Math.abs(x.net_pnl)));
                const up = r.net_pnl >= 0;
                return (
                  <div
                    key={r.reason}
                    className="grid grid-cols-[150px_1fr_140px] items-center gap-3 text-xs"
                  >
                    <span className="truncate text-foreground">{title(r.reason)}</span>
                    <div className="h-2 rounded-full bg-background/60">
                      <div
                        className={cn('h-full rounded-full', up ? 'bg-accent/70' : 'bg-destructive/70')}
                        style={{ width: `${(Math.abs(r.net_pnl) / max) * 100}%` }}
                      />
                    </div>
                    <span className="text-right font-mono tabular-nums text-muted-foreground">
                      <span className={tone(r.net_pnl)}>{signed(r.net_pnl)}</span> · {r.trades}t ·{' '}
                      {r.wins}w
                    </span>
                  </div>
                );
              })}
            </div>
          </section>

          {/* Worst symbols */}
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
              Por token · pior primeiro
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-left text-[10px] uppercase tracking-wider text-muted-foreground">
                    <th className="pb-2 pr-3 font-medium">Símbolo</th>
                    <th className="pb-2 pr-3 text-right font-medium">net PnL</th>
                    <th className="pb-2 pr-3 text-right font-medium">Trades</th>
                    <th className="pb-2 text-right font-medium">Win%</th>
                  </tr>
                </thead>
                <tbody className="font-mono tabular-nums">
                  {data.worst_symbols.map(s => (
                    <tr key={s.symbol} className="border-t border-border/50">
                      <td className="py-1.5 pr-3 text-foreground">{s.symbol}</td>
                      <td className={cn('py-1.5 pr-3 text-right', tone(s.realized_pnl))}>
                        {signed(s.realized_pnl)}
                      </td>
                      <td className="py-1.5 pr-3 text-right text-muted-foreground">{s.trades}</td>
                      <td className="py-1.5 text-right text-muted-foreground">
                        {(s.win_rate * 100).toFixed(0)}%
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        </>
      ) : null}
    </div>
  );
}
