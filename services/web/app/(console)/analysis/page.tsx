'use client';

import { useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { ConfirmAction } from '@/components/system/ConfirmAction';
import { cn } from '@/lib/utils';

type Override = { path: string; value: string };
type Measured = {
  delta_net_pnl?: number;
  baseline_net_pnl?: number;
  variant_net_pnl?: number;
  corpus_ticks?: number;
};
type Recommendation = {
  overrides?: Override[];
  measured?: Measured;
  rationale?: string;
  expected_effect?: string;
};
type ReasonRow = {
  reason: string;
  trades?: number;
  net_pnl?: number;
  win_pct?: number;
};
type SweepVariant = { label: string; delta_net_pnl?: number };
type Sweep = { variants?: SweepVariant[] };
type Review = {
  verdict?: 'change_recommended' | 'no_improvement';
  provider_model?: string;
  recommendation?: Recommendation | null;
  final_message?: string | null;
  diagnostics?: { by_close_reason?: ReasonRow[] };
  sweeps?: Sweep[];
  sweeps_run?: number;
};
type ReviewItem = {
  id: number;
  created_at: string;
  trade_count: number;
  applied_version_id: number | null;
  review: Review;
};

const num = (v?: number, d = 2) =>
  typeof v === 'number' && !Number.isNaN(v) ? v.toFixed(d) : '—';
const signed = (v?: number, d = 2) =>
  typeof v === 'number' && !Number.isNaN(v)
    ? `${v >= 0 ? '+' : '−'}${Math.abs(v).toFixed(d)}`
    : '—';
const title = (s?: string) =>
  (s || '')
    .replaceAll('_', ' ')
    .replace(/\b\w/g, c => c.toUpperCase());
const shortPath = (p: string) => p.replace(/^mode_profiles\.[A-Z]+\./, '');

function relTime(iso: string) {
  const ms = Date.parse(iso);
  if (Number.isNaN(ms)) return '—';
  const s = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

function VerdictBanner({
  item,
  onApply,
}: {
  item: ReviewItem;
  onApply: (reviewId: number) => Promise<void>;
}) {
  const review = item.review;
  const rec = review.recommendation;
  if (review.verdict === 'change_recommended' && rec) {
    const d = rec.measured?.delta_net_pnl;
    const base = rec.measured?.baseline_net_pnl;
    const variant = rec.measured?.variant_net_pnl;
    const applied = item.applied_version_id != null;
    return (
      <div className="rounded-lg border border-accent/40 bg-accent/10 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="text-[10px] uppercase tracking-[0.2em] text-accent">
            Recommended change · engine-verified
          </div>
          {applied ? (
            <span className="shrink-0 rounded-md border border-accent/40 bg-accent/10 px-2 py-0.5 text-[11px] text-accent">
              Applied as v{item.applied_version_id}
            </span>
          ) : (
            <ConfirmAction
              label="Aplicar"
              confirmLabel="Apply live"
              tone="danger"
              onConfirm={() => onApply(item.id)}
            />
          )}
        </div>
        <div className="mt-2 flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <div className="font-mono text-sm text-foreground">
            {(rec.overrides ?? []).map((o, i) => (
              <span key={i} className="mr-3 inline-block">
                <span className="text-muted-foreground">{shortPath(o.path)}</span>{' '}
                <span className="font-semibold text-accent">= {o.value}</span>
              </span>
            ))}
          </div>
        </div>
        <div className="mt-2 font-mono text-sm tabular-nums text-foreground">
          measured Δ{' '}
          <span className="font-semibold text-accent">{signed(d, 4)}</span>
          {typeof base === 'number' && typeof variant === 'number' && (
            <span className="text-muted-foreground">
              {' '}
              (net {signed(base)} → {signed(variant)})
            </span>
          )}
        </div>
        {rec.rationale && (
          <p className="mt-2 text-xs text-muted-foreground">{rec.rationale}</p>
        )}
      </div>
    );
  }
  return (
    <div className="rounded-lg border border-border bg-secondary/40 p-4">
      <div className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        No change recommended
      </div>
      <p className="mt-2 text-sm text-foreground/80">
        The agent ran the diagnostics and {review.sweeps_run ?? 0} backtest
        sweep{review.sweeps_run === 1 ? '' : 's'} and found no change that
        improves net PnL on the recent corpus — the config is at a local optimum.
      </p>
      {review.final_message && (
        <p className="mt-2 text-xs text-muted-foreground">
          {review.final_message}
        </p>
      )}
    </div>
  );
}

function Evidence({ rows }: { rows: ReasonRow[] }) {
  if (rows.length === 0) return null;
  const max = Math.max(1, ...rows.map(r => Math.abs(r.net_pnl ?? 0)));
  return (
    <div>
      <div className="mb-2 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Evidence · recent close-reason PnL
      </div>
      <div className="space-y-1.5">
        {rows.map(r => {
          const net = r.net_pnl ?? 0;
          const up = net >= 0;
          return (
            <div
              key={r.reason}
              className="grid grid-cols-[150px_1fr_70px] items-center gap-3 text-xs"
            >
              <span className="truncate text-foreground">
                {title(r.reason)}
              </span>
              <div className="h-2 rounded-full bg-background/60">
                <div
                  className={cn(
                    'h-full rounded-full',
                    up ? 'bg-accent/70' : 'bg-destructive/70'
                  )}
                  style={{ width: `${(Math.abs(net) / max) * 100}%` }}
                />
              </div>
              <span
                className={cn(
                  'text-right font-mono tabular-nums',
                  up ? 'text-accent' : 'text-destructive'
                )}
              >
                {signed(net)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function BacktestRuns({ sweeps }: { sweeps: Sweep[] }) {
  const rows = sweeps
    .flatMap(s => s.variants ?? [])
    .filter(v => v.label);
  if (rows.length === 0) return null;
  return (
    <div>
      <div className="mb-2 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Backtest · what the AI tried
      </div>
      <div className="space-y-1">
        {rows.map((v, i) => {
          const d = v.delta_net_pnl ?? 0;
          const tone =
            d > 1e-9
              ? 'text-accent'
              : d < -1e-9
                ? 'text-destructive'
                : 'text-muted-foreground';
          return (
            <div
              key={i}
              className="flex items-center justify-between gap-3 border-b border-border/50 pb-1 text-xs last:border-0"
            >
              <span className="truncate font-mono text-muted-foreground">
                {v.label
                  .split(',')
                  .map(p => shortPath(p.trim()))
                  .join(', ')}
              </span>
              <span className={cn('shrink-0 font-mono tabular-nums', tone)}>
                Δ {signed(d, 4)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function ReviewBody({
  item,
  onApply,
}: {
  item: ReviewItem;
  onApply: (reviewId: number) => Promise<void>;
}) {
  const review = item.review;
  return (
    <div className="space-y-4">
      <VerdictBanner item={item} onApply={onApply} />
      <Evidence rows={review.diagnostics?.by_close_reason ?? []} />
      <BacktestRuns sweeps={review.sweeps ?? []} />
    </div>
  );
}

export default function AnalysisPage() {
  const { data, loading, error, refresh } = useDashboard<{ items: ReviewItem[] }>(
    '/api/v1/tuning-reviews?limit=20',
    { refreshInterval: 30000 }
  );
  const items = data?.items ?? [];
  const latest = items[0];
  const history = items.slice(1);
  const [openId, setOpenId] = useState<string | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);

  async function onApply(reviewId: number) {
    setApplyError(null);
    const res = await fetch('/api/config', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        kind: 'apply-review',
        payload: { review_id: reviewId },
      }),
    });
    const body = await res.json().catch(() => null);
    if (!res.ok || body?.ok === false) {
      const msg = body?.message || body?.error || `HTTP ${res.status}`;
      setApplyError(msg);
      throw new Error(msg);
    }
    await refresh();
  }

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-foreground">
          Analysis
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Automated tuning review — every ~10 trades the AI agent reads the
          evidence, runs deterministic backtests, and proposes a config change it
          can prove. No guesses.
        </p>
      </div>

      {(error || applyError) && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {applyError || error}
        </div>
      )}

      {loading && items.length === 0 ? (
        <div className="h-72 animate-pulse rounded-xl border border-border bg-card" />
      ) : !latest ? (
        <div className="flex h-48 items-center justify-center rounded-xl border border-border bg-card text-sm text-muted-foreground">
          No tuning review yet — the first one runs after the next batch of
          trades.
        </div>
      ) : (
        <>
          <section className="rounded-xl border border-border bg-card p-5">
            <div className="mb-4 flex flex-wrap items-baseline justify-between gap-2">
              <h2 className="text-base font-semibold text-foreground">
                Latest review
              </h2>
              <span className="font-mono text-xs text-muted-foreground">
                as of trade #{latest.trade_count} · {relTime(latest.created_at)}
                {latest.review.provider_model &&
                  ` · ${latest.review.provider_model.split(' ')[0]}`}
              </span>
            </div>
            <ReviewBody item={latest} onApply={onApply} />
          </section>

          {history.length > 0 && (
            <section className="rounded-xl border border-border bg-card p-5">
              <div className="mb-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                History
              </div>
              <div className="divide-y divide-border">
                {history.map(item => {
                  const id = item.created_at;
                  const open = openId === id;
                  const changed =
                    item.review.verdict === 'change_recommended';
                  return (
                    <div key={id} className="py-2">
                      <button
                        type="button"
                        onClick={() => setOpenId(open ? null : id)}
                        className="flex w-full items-center justify-between gap-3 text-left text-sm"
                      >
                        <span className="flex items-center gap-3">
                          <span className="font-mono text-muted-foreground">
                            #{item.trade_count}
                          </span>
                          <span className="text-xs text-muted-foreground">
                            {relTime(item.created_at)}
                          </span>
                        </span>
                        <span
                          className={cn(
                            'rounded-md border px-2 py-0.5 text-[11px]',
                            changed
                              ? 'border-accent/40 bg-accent/10 text-accent'
                              : 'border-border bg-secondary/40 text-muted-foreground'
                          )}
                        >
                          {changed
                            ? `recommended ${(
                                item.review.recommendation?.overrides ?? []
                              )
                                .map(o => shortPath(o.path))
                                .join(', ') || 'a change'}`
                            : 'no change'}
                        </span>
                      </button>
                      {open && (
                        <div className="mt-3">
                          <ReviewBody item={item} onApply={onApply} />
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </section>
          )}
        </>
      )}
    </div>
  );
}
