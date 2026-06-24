'use client';

import { cn } from '@/lib/utils';
import { RunBar } from '@/components/analysis/TuningTab';
import { signed, SubstitutionCard, tone, TuningState } from '@/components/analysis/tuningShared';

function SymbolTable({ rows, dropCandidate }: { rows: { symbol: string; trades: number; net_pnl: number; wins: number; win_rate_pct: number; enabled: boolean }[]; dropCandidate: string | null }) {
  return (
    <section className="rounded-xl border border-border bg-card p-5">
      <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Performance por token · pior primeiro (scoreboard de pruning)
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-[10px] uppercase tracking-wider text-muted-foreground">
              <th className="pb-2 pr-3 font-medium">Símbolo</th>
              <th className="pb-2 pr-3 text-right font-medium">net PnL</th>
              <th className="pb-2 pr-3 text-right font-medium">Trades</th>
              <th className="pb-2 pr-3 text-right font-medium">Win%</th>
              <th className="pb-2 text-right font-medium">Estado</th>
            </tr>
          </thead>
          <tbody className="font-mono tabular-nums">
            {rows.map(r => (
              <tr
                key={r.symbol}
                className={cn(
                  'border-t border-border/50',
                  r.symbol === dropCandidate && 'bg-destructive/5',
                )}
              >
                <td className="py-1.5 pr-3 text-foreground">
                  {r.symbol}
                  {r.symbol === dropCandidate && (
                    <span className="ml-2 rounded border border-destructive/40 bg-destructive/10 px-1 py-0.5 text-[9px] uppercase tracking-wide text-destructive">
                      drop?
                    </span>
                  )}
                </td>
                <td className={cn('py-1.5 pr-3 text-right font-semibold', tone(r.net_pnl))}>
                  {signed(r.net_pnl)}
                </td>
                <td className="py-1.5 pr-3 text-right text-muted-foreground">{r.trades}</td>
                <td className="py-1.5 pr-3 text-right text-muted-foreground">
                  {r.win_rate_pct.toFixed(0)}%
                </td>
                <td className="py-1.5 text-right">
                  <span
                    className={cn(
                      'rounded border px-1.5 py-0.5 text-[10px] uppercase tracking-wide',
                      r.enabled
                        ? 'border-accent/40 bg-accent/10 text-accent'
                        : 'border-border bg-secondary/40 text-muted-foreground',
                    )}
                  >
                    {r.enabled ? 'ativo' : 'off'}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export default function SymbolTab({ tuning }: { tuning: TuningState }) {
  const { data, loading, error } = tuning;
  return (
    <div className="space-y-5">
      <RunBar
        tuning={tuning}
        blurb="Performance realizada por token sobre o corpus de backtest (mesmos dados da aba Tuning). Ranqueado do pior para o melhor, com a hipótese de substituição."
      />

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && <div className="h-40 animate-pulse rounded-xl border border-border bg-card" />}

      {data && !loading && (
        <>
          <SymbolTable rows={data.by_symbol} dropCandidate={data.substitution.drop_candidate} />
          <SubstitutionCard sub={data.substitution} />
        </>
      )}

      {!data && !loading && !error && (
        <div className="flex h-40 items-center justify-center rounded-xl border border-dashed border-border bg-card text-sm text-muted-foreground">
          Clique em “Gerar análise” para carregar a performance por token.
        </div>
      )}
    </div>
  );
}
