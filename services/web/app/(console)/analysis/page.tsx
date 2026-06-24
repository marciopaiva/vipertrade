'use client';

import { useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { cn } from '@/lib/utils';
import TuningTab from '@/components/analysis/TuningTab';
import SymbolTab from '@/components/analysis/SymbolTab';
import { useTuning } from '@/components/analysis/tuningShared';

type Summary = {
  closed_trades?: number;
  total_pnl_usdt?: number;
  avg_pnl_pct?: number;
  win_rate_pct?: number;
};
type Expectancy = {
  payoff_ratio?: number;
  expectancy_usdt?: number;
  expectancy_pct?: number;
  winning_trades?: number;
  losing_trades?: number;
};
type Breakdown = {
  name: string;
  trades?: number;
  pnl_usdt?: number;
  avg_pnl_pct?: number;
};
type Recommendation = {
  recommendation_id: string;
  severity?: string;
  confidence?: string;
  recommendation: string;
  evidence?: string;
  expected_tradeoff?: string;
};
type Hypothesis = {
  hypothesis_id: string;
  priority?: string;
  confidence?: string;
  hypothesis: string;
  evidence?: string;
  observe?: string;
};
type Blocker = { reason: string; total?: number };
type Analysis = {
  generated_at?: string;
  lookback_hours?: number;
  summary?: Summary;
  expectancy?: Expectancy;
  by_close_reason?: Breakdown[];
  recommendations?: Recommendation[];
  hypotheses?: Hypothesis[];
  top_entry_blockers?: Blocker[];
  heuristic_summary?: string;
  tupa_error?: string;
};

const num = (v?: number, d = 2) =>
  typeof v === 'number' && !Number.isNaN(v) ? v.toFixed(d) : '—';
const signed = (v?: number, d = 2) =>
  typeof v === 'number' && !Number.isNaN(v)
    ? `${v >= 0 ? '+' : '−'}${Math.abs(v).toFixed(d)}`
    : '—';
const title = (s?: string) =>
  (s || '').replaceAll('_', ' ').replace(/\b\w/g, c => c.toUpperCase());

function relTime(iso?: string) {
  if (!iso) return '—';
  const ms = Date.parse(iso);
  if (Number.isNaN(ms)) return '—';
  const s = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

const toneFor = (level?: string) => {
  const l = (level || '').toLowerCase();
  if (l === 'high' || l === 'critical')
    return 'border-destructive/40 bg-destructive/10 text-destructive';
  if (l === 'medium' || l === 'warning')
    return 'border-accent/40 bg-accent/10 text-accent';
  return 'border-border bg-secondary/40 text-muted-foreground';
};

function Kpi({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div className="rounded-lg border border-border bg-card p-3">
      <div className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
        {label}
      </div>
      <div className={cn('mt-1 font-mono text-lg tabular-nums', tone ?? 'text-foreground')}>
        {value}
      </div>
    </div>
  );
}

function CloseReasonEvidence({ rows }: { rows: Breakdown[] }) {
  if (rows.length === 0) return null;
  const max = Math.max(1, ...rows.map(r => Math.abs(r.pnl_usdt ?? 0)));
  return (
    <section className="rounded-xl border border-border bg-card p-5">
      <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Evidence · PnL by close reason
      </div>
      <div className="space-y-1.5">
        {rows.map(r => {
          const net = r.pnl_usdt ?? 0;
          const up = net >= 0;
          return (
            <div
              key={r.name}
              className="grid grid-cols-[170px_1fr_120px] items-center gap-3 text-xs"
            >
              <span className="truncate text-foreground">{title(r.name)}</span>
              <div className="h-2 rounded-full bg-background/60">
                <div
                  className={cn('h-full rounded-full', up ? 'bg-accent/70' : 'bg-destructive/70')}
                  style={{ width: `${(Math.abs(net) / max) * 100}%` }}
                />
              </div>
              <span className="text-right font-mono tabular-nums text-muted-foreground">
                <span className={up ? 'text-accent' : 'text-destructive'}>{signed(net)}</span>
                {' · '}
                {r.trades ?? 0}t
              </span>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function Recommendations({ rows }: { rows: Recommendation[] }) {
  if (rows.length === 0) return null;
  return (
    <section className="rounded-xl border border-border bg-card p-5">
      <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Recommendations · advisory (apply manually via backtest)
      </div>
      <div className="space-y-3">
        {rows.map(r => (
          <div key={r.recommendation_id} className="rounded-lg border border-border bg-secondary/30 p-3">
            <div className="flex items-start justify-between gap-3">
              <p className="text-sm font-medium text-foreground">{r.recommendation}</p>
              <span className={cn('shrink-0 rounded-md border px-2 py-0.5 text-[10px] uppercase', toneFor(r.severity))}>
                {r.severity || 'info'}
              </span>
            </div>
            {r.evidence && <p className="mt-1.5 text-xs text-muted-foreground">{r.evidence}</p>}
            {r.expected_tradeoff && (
              <p className="mt-1 text-xs text-muted-foreground">
                <span className="text-foreground/70">Tradeoff:</span> {r.expected_tradeoff}
              </p>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

function Hypotheses({ rows }: { rows: Hypothesis[] }) {
  if (rows.length === 0) return null;
  return (
    <section className="rounded-xl border border-border bg-card p-5">
      <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Hypotheses to watch
      </div>
      <div className="space-y-3">
        {rows.map(h => (
          <div key={h.hypothesis_id} className="rounded-lg border border-border bg-secondary/30 p-3">
            <div className="flex items-start justify-between gap-3">
              <p className="text-sm font-medium text-foreground">{h.hypothesis}</p>
              <span className={cn('shrink-0 rounded-md border px-2 py-0.5 text-[10px] uppercase', toneFor(h.priority))}>
                {h.priority || 'low'}
              </span>
            </div>
            {h.evidence && <p className="mt-1.5 text-xs text-muted-foreground">{h.evidence}</p>}
            {h.observe && (
              <p className="mt-1 text-xs text-muted-foreground">
                <span className="text-foreground/70">Observe:</span> {h.observe}
              </p>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

type TabId = 'tuning' | 'symbol' | 'diag';

export default function AnalysisPage() {
  const [tab, setTab] = useState<TabId>('tuning');
  const tuning = useTuning();
  const { data, loading, error } = useDashboard<Analysis>('/api/analysis', {
    refreshInterval: 30000,
  });

  const summary = data?.summary;
  const exp = data?.expectancy;
  const blockers = (data?.top_entry_blockers ?? []).slice(0, 6);
  const unavailable = data?.tupa_error && !summary?.closed_trades;

  const tabs: Array<{ id: TabId; label: string }> = [
    { id: 'tuning', label: 'Tuning (IA)' },
    { id: 'symbol', label: 'Por Token' },
    { id: 'diag', label: 'Diagnóstico' },
  ];

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-foreground">Analysis</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Tuning assistido por IA sobre o grid de backtest determinístico, performance
          por token, e diagnóstico descritivo. Mudanças são aplicadas manualmente.
        </p>
      </div>

      <div className="flex gap-1 border-b border-border">
        {tabs.map(t => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            className={cn(
              '-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors',
              tab === t.id
                ? 'border-accent text-foreground'
                : 'border-transparent text-muted-foreground hover:text-foreground',
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'tuning' && <TuningTab tuning={tuning} />}

      {tab === 'symbol' && <SymbolTab tuning={tuning} />}

      {tab === 'diag' && (
        <div className="space-y-5">
          <p className="text-xs text-muted-foreground">
            Diagnóstico descritivo dos últimos {data?.lookback_hours ?? 24}h — determinístico,
            sem LLM.{data?.generated_at && ` · atualizado ${relTime(data.generated_at)}`}
          </p>

          {error && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </div>
          )}

          {loading && !data ? (
            <div className="h-72 animate-pulse rounded-xl border border-border bg-card" />
          ) : unavailable ? (
            <div className="flex h-48 items-center justify-center rounded-xl border border-border bg-card text-sm text-muted-foreground">
              Analyst unavailable — no closed trades in the window yet.
            </div>
          ) : (
            <>
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
            <Kpi label="Closed trades" value={num(summary?.closed_trades, 0)} />
            <Kpi
              label="Win rate"
              value={`${num(summary?.win_rate_pct, 1)}%`}
              tone={(summary?.win_rate_pct ?? 0) >= 50 ? 'text-accent' : 'text-foreground'}
            />
            <Kpi
              label="Net PnL"
              value={signed(summary?.total_pnl_usdt)}
              tone={(summary?.total_pnl_usdt ?? 0) >= 0 ? 'text-accent' : 'text-destructive'}
            />
            <Kpi
              label="Expectancy"
              value={signed(exp?.expectancy_usdt)}
              tone={(exp?.expectancy_usdt ?? 0) >= 0 ? 'text-accent' : 'text-destructive'}
            />
            <Kpi label="Payoff ratio" value={num(exp?.payoff_ratio)} />
          </div>

          {data?.heuristic_summary && (
            <section className="rounded-xl border border-border bg-card p-5">
              <div className="mb-2 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Operational note
              </div>
              <p className="whitespace-pre-line text-sm leading-relaxed text-foreground/90">
                {data.heuristic_summary}
              </p>
            </section>
          )}

          <CloseReasonEvidence rows={data?.by_close_reason ?? []} />
          <Recommendations rows={data?.recommendations ?? []} />
          <Hypotheses rows={data?.hypotheses ?? []} />

          {blockers.length > 0 && (
            <section className="rounded-xl border border-border bg-card p-5">
              <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Top entry blockers
              </div>
              <div className="flex flex-wrap gap-2">
                {blockers.map(b => (
                  <span
                    key={b.reason}
                    className="rounded-md border border-border bg-secondary/40 px-2.5 py-1 text-xs text-muted-foreground"
                  >
                    {title(b.reason)}{' '}
                    <span className="font-mono text-foreground">{b.total ?? 0}</span>
                  </span>
                ))}
              </div>
            </section>
          )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
