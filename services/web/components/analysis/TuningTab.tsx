'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import {
  ClassBadge,
  GridVariant,
  signed,
  SubstitutionCard,
  tone,
  TuningState,
} from '@/components/analysis/tuningShared';

function DiffBlock({ variant }: { variant: GridVariant }) {
  const [copied, setCopied] = useState(false);
  const leaf = variant.path.split('.').pop() ?? variant.path;
  const snippet = `# config/trading/pairs.yaml → global.${variant.path}\n${leaf}: ${variant.value}`;
  return (
    <div className="relative mt-3 rounded-lg border border-border bg-background/60 p-3">
      <button
        type="button"
        onClick={() => {
          void navigator.clipboard.writeText(snippet);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
        className="absolute right-2 top-2 rounded-md border border-border bg-secondary/40 px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground hover:text-foreground"
      >
        {copied ? 'copiado ✓' : 'copiar'}
      </button>
      <pre className="overflow-x-auto whitespace-pre-wrap font-mono text-xs text-foreground/90">
        {snippet}
      </pre>
    </div>
  );
}

function RecommendationCard({ rec }: { rec: GridVariant | null }) {
  return (
    <section className="rounded-xl border border-accent/30 bg-accent/5 p-5">
      <div className="mb-2 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Recomendação · aplicar manualmente (sem auto-apply)
      </div>
      {rec ? (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-sm text-foreground">{rec.path}</span>
            <span className="font-mono text-sm font-semibold text-foreground">= {rec.value}</span>
            <ClassBadge klass={rec.class} />
            <span className={cn('font-mono text-sm tabular-nums', tone(rec.delta_net_pnl))}>
              Δ {signed(rec.delta_net_pnl)}
            </span>
          </div>
          <p className="mt-1.5 text-xs text-muted-foreground">
            Melhor variante <span className="text-accent">alpha</span> com delta positivo no corpus.
            Variantes <span className="text-amber-500">exposure</span> (só reduzem tamanho) nunca são
            recomendadas como tuning.
          </p>
          <DiffBlock variant={rec} />
        </>
      ) : (
        <p className="text-sm text-muted-foreground">
          Nenhuma melhoria de <span className="text-accent">alpha</span> no corpus atual — manter a
          config. (Variantes de exposição não contam como melhoria de estratégia.)
        </p>
      )}
    </section>
  );
}

function GridTable({ variants }: { variants: GridVariant[] }) {
  return (
    <section className="rounded-xl border border-border bg-card p-5">
      <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Grid determinístico · ordenado por Δ net PnL (sinal explícito)
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-[10px] uppercase tracking-wider text-muted-foreground">
              <th className="pb-2 pr-3 font-medium">Eixo</th>
              <th className="pb-2 pr-3 font-medium">Valor</th>
              <th className="pb-2 pr-3 font-medium">Classe</th>
              <th className="pb-2 pr-3 text-right font-medium">Δ net PnL</th>
              <th className="pb-2 pr-3 text-right font-medium">net PnL</th>
              <th className="pb-2 text-right font-medium">W/L</th>
            </tr>
          </thead>
          <tbody className="font-mono tabular-nums">
            {variants.map(v => (
              <tr key={`${v.path}-${v.value}`} className="border-t border-border/50">
                <td className="py-1.5 pr-3 text-foreground/80">{v.axis}</td>
                <td className="py-1.5 pr-3 text-foreground">{v.value}</td>
                <td className="py-1.5 pr-3">
                  <ClassBadge klass={v.class} />
                </td>
                <td className={cn('py-1.5 pr-3 text-right font-semibold', tone(v.delta_net_pnl))}>
                  {signed(v.delta_net_pnl)}
                </td>
                <td className={cn('py-1.5 pr-3 text-right', tone(v.net_pnl))}>{signed(v.net_pnl)}</td>
                <td className="py-1.5 text-right text-muted-foreground">
                  {v.wins}/{v.losses}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function KpiBox({ label, value, tone: t }: { label: string; value: string; tone?: string }) {
  return (
    <div className="rounded-lg border border-border bg-card p-3">
      <div className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">{label}</div>
      <div className={cn('mt-1 font-mono text-lg tabular-nums', t ?? 'text-foreground')}>{value}</div>
    </div>
  );
}

// Run/regenerate bar shared by both tuning tabs.
export function RunBar({ tuning, blurb }: { tuning: TuningState; blurb: string }) {
  return (
    <section className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-card p-5">
      <p className="max-w-2xl text-xs text-muted-foreground">{blurb}</p>
      <Button onClick={() => void tuning.run()} disabled={tuning.loading}>
        {tuning.loading ? 'Rodando…' : tuning.data ? 'Gerar novamente' : 'Gerar análise'}
      </Button>
    </section>
  );
}

export default function TuningTab({ tuning }: { tuning: TuningState }) {
  const { data, loading, error } = tuning;
  return (
    <div className="space-y-5">
      <RunBar
        tuning={tuning}
        blurb="Grid de backtest determinístico (paths/PnL calculados no Rust) sobre o corpus de auditoria. On-demand. ⚠️ O backtest NÃO modela o trailing ao vivo (advice/min_hold) — confie nos eixos de entrada; valide trailing pela aba Ao Vivo."
      />

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && <div className="h-40 animate-pulse rounded-xl border border-border bg-card" />}

      {data && !loading && (
        <>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <KpiBox label="Corpus ticks" value={data.corpus_ticks.toLocaleString()} />
            <KpiBox
              label="Baseline net PnL"
              value={signed(data.baseline.net_pnl)}
              tone={tone(data.baseline.net_pnl)}
            />
            <KpiBox label="Win rate" value={`${data.baseline.win_rate_pct.toFixed(1)}%`} />
            <KpiBox label="Closed" value={String(data.baseline.closed)} />
          </div>

          <RecommendationCard rec={data.recommended} />
          <GridTable variants={data.variants} />
          <SubstitutionCard sub={data.substitution} />
        </>
      )}

      {!data && !loading && !error && (
        <div className="flex h-40 items-center justify-center rounded-xl border border-dashed border-border bg-card text-sm text-muted-foreground">
          Clique em “Gerar análise” para rodar o grid de tuning.
        </div>
      )}
    </div>
  );
}
