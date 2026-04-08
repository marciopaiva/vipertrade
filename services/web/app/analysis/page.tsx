'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { ViperTradeLogo } from '@/components/ViperTradeLogo';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

type BreakdownItem = {
  name: string;
  trades: number;
  pnl_usdt: number;
  avg_pnl_pct: number;
  avg_duration_s: number;
};

type BlockerItem = {
  reason: string;
  total: number;
};

type AnalystEvaluationSignal = {
  reason?: string;
  severity?: 'pass' | 'warn' | 'fail';
  dominant_gate?: string;
  symbol?: string;
  recommendation?: string;
  top_reason?: string;
  thesis_invalidated_pct?: number;
  trailing_stop_pct?: number;
};

type ThesisReasonItem = {
  reason: string;
  total: number;
};

type AiAnalystData = {
  generated_at?: string;
  lookback_hours?: number;
  heuristic_summary?: string;
  llm_summary?: string | null;
  tupa_error?: string | null;
  summary?: {
    closed_trades?: number;
    total_pnl_usdt?: number;
    avg_pnl_pct?: number;
    avg_duration_s?: number;
    win_rate_pct?: number;
  };
  expectancy?: {
    winning_trades?: number;
    losing_trades?: number;
    neutral_trades?: number;
    avg_win_usdt?: number;
    avg_win_pct?: number;
    avg_loss_usdt?: number;
    avg_loss_pct?: number;
    payoff_ratio?: number;
    expectancy_usdt?: number;
    expectancy_pct?: number;
  };
  comparative_diagnostics?: {
    status?: string;
    reasons?: string[];
    current_window_hours?: number;
    previous_window_hours?: number;
    closed_trades?: { current?: number; previous?: number; delta?: number };
    win_rate_pct?: { current?: number; previous?: number; delta?: number };
    expectancy_pct?: { current?: number; previous?: number; delta?: number };
    payoff_ratio?: { current?: number; previous?: number; delta?: number };
    thesis_invalidated_pct?: { current?: number; previous?: number; delta?: number };
    trailing_stop_pct?: { current?: number; previous?: number; delta?: number };
    long_avg_pnl_pct?: { current?: number; previous?: number; delta?: number };
    short_avg_pnl_pct?: { current?: number; previous?: number; delta?: number };
  };
  recommendations?: Array<{
    recommendation_id?: string;
    severity?: 'pass' | 'warn' | 'fail' | 'info' | string;
    confidence?: string;
    recommendation?: string;
    evidence?: string;
    expected_tradeoff?: string;
  }>;
  hypotheses?: Array<{
    hypothesis_id?: string;
    priority?: string;
    confidence?: string;
    hypothesis?: string;
    evidence?: string;
    observe?: string;
    success_condition?: string;
    failure_condition?: string;
  }>;
  symbol_diagnostics?: Array<{
    symbol?: string;
    status?: string;
    recommendation?: string;
    confidence?: string;
    trades?: number;
    avg_pnl_pct?: number;
    thesis_invalidated_trades?: number;
    trailing_stop_trades?: number;
    avg_thesis_pnl_pct?: number;
    avg_trailing_pnl_pct?: number;
  }>;
  regime_diagnostics?: {
    status?: string;
    regime?: string;
    confidence?: string;
    directional_bias?: string;
    dominant_gate?: string;
    exit_profile?: string;
    evidence?: string[];
  };
  execution_advice?: {
    market_state?: string;
    entry_action?: string;
    exit_action?: string;
    size_action?: string;
    directional_bias?: string;
    confidence?: string;
    summary?: string;
    evidence?: string[];
    priority_actions?: string[];
  };
  active_position_advice?: Array<{
    symbol?: string;
    side?: string;
    action?: string;
    confidence?: string;
    market_state?: string;
    pnl_pct_estimate?: number;
    duration_minutes?: number;
    summary?: string;
    evidence?: string[];
    risk_flags?: string[];
  }>;
  by_close_reason?: BreakdownItem[];
  by_side?: BreakdownItem[];
  by_symbol?: BreakdownItem[];
  top_entry_blockers?: BlockerItem[];
  thesis_invalidation_breakdown?: ThesisReasonItem[];
  tupa_evaluation?: {
    exit_pressure?: AnalystEvaluationSignal;
    directional_bias?: AnalystEvaluationSignal;
    entry_pressure?: AnalystEvaluationSignal;
    thesis_quality?: AnalystEvaluationSignal;
    symbol_risk?: AnalystEvaluationSignal;
  };
};

type DashboardResponse = {
  ai_analyst?: AiAnalystData;
  warnings?: string[];
  positions?: { items?: Array<{ symbol: string; side: string; notional_usdt: number }> };
  events?: { items?: Array<{ event_type?: string; symbol?: string; data?: any }> };
  market_signals?: { items?: any[] | Record<string, any> };
};

function usd(value: number | null | undefined) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 2 }).format(value);
}

function num(value: number | null | undefined, digits = 2) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return value.toFixed(digits);
}

function titleCase(value: string | null | undefined) {
  if (!value) return '';
  return value
    .replaceAll('_', ' ')
    .toLowerCase()
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

function toneClasses(severity?: string) {
  if (severity === 'fail') {
    return {
      badge: 'border-red-500/35 bg-red-500/10 text-red-300',
      text: 'text-red-300',
    };
  }
  if (severity === 'warn') {
    return {
      badge: 'border-amber-500/35 bg-amber-500/10 text-amber-300',
      text: 'text-amber-300',
    };
  }
  return {
    badge: 'border-emerald-500/35 bg-emerald-500/10 text-emerald-300',
    text: 'text-emerald-300',
  };
}

function comparativeTone(status?: string) {
  if (status === 'regressed') return toneClasses('fail');
  if (status === 'mixed' || status === 'insufficient_baseline') return toneClasses('warn');
  return toneClasses('pass');
}

function BreakdownTable({
  title,
  items,
  nameLabel,
}: {
  title: string;
  items: BreakdownItem[];
  nameLabel: string;
}) {
  return (
    <Card className="bg-panel/50 border-border">
      <CardHeader className="pb-2">
        <CardTitle className="text-base">{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          {items.length === 0 ? (
            <div className="text-sm text-muted-foreground">No data available.</div>
          ) : (
            items.map((item) => (
              <div key={item.name} className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="text-xs uppercase tracking-[0.18em] text-slate-500">{nameLabel}</div>
                    <div className="mt-1 text-sm font-semibold text-slate-100">{titleCase(item.name)}</div>
                  </div>
                  <div className={cn('text-sm font-semibold', item.pnl_usdt >= 0 ? 'text-emerald-300' : 'text-red-300')}>
                    {usd(item.pnl_usdt)}
                  </div>
                </div>
                <div className="mt-3 grid grid-cols-3 gap-3 text-xs text-slate-400">
                  <div>
                    <div className="uppercase tracking-[0.16em] text-slate-500">Trades</div>
                    <div className="mt-1 text-slate-200">{item.trades}</div>
                  </div>
                  <div>
                    <div className="uppercase tracking-[0.16em] text-slate-500">Avg PnL</div>
                    <div className={cn('mt-1', item.avg_pnl_pct >= 0 ? 'text-emerald-300' : 'text-red-300')}>
                      {num(item.avg_pnl_pct)}%
                    </div>
                  </div>
                  <div>
                    <div className="uppercase tracking-[0.16em] text-slate-500">Avg Duration</div>
                    <div className="mt-1 text-slate-200">{num(item.avg_duration_s, 0)}s</div>
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export default function AnalysisPage() {
  const [payload, setPayload] = useState<DashboardResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAnalysis = useCallback(async () => {
    try {
      const res = await fetch('/api/dashboard', { cache: 'no-store' });
      const data = await res.json();
      setPayload(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAnalysis();
    const interval = setInterval(fetchAnalysis, 15000);
    return () => clearInterval(interval);
  }, [fetchAnalysis]);

  const analyst = useMemo(() => payload?.ai_analyst, [payload]);
  const exitTone = toneClasses(analyst?.tupa_evaluation?.exit_pressure?.severity);
  const symbolTone = toneClasses(analyst?.tupa_evaluation?.symbol_risk?.severity);
  const comparative = analyst?.comparative_diagnostics;
  const comparativeToneState = comparativeTone(comparative?.status);
  const regime = analyst?.regime_diagnostics;
  const regimeTone = comparativeTone(regime?.status);
  const executionAdvice = analyst?.execution_advice;
  const executionTone = comparativeTone(
    executionAdvice?.market_state === 'defensive'
      ? 'regressed'
      : executionAdvice?.market_state === 'constructive'
        ? 'improved'
        : 'mixed',
  );
  return (
    <div className="min-h-screen bg-background">
      <header className="border-b border-border bg-panel/50 backdrop-blur-sm sticky top-0 z-50">
        <div className="container mx-auto px-4 py-4">
          <div className="flex items-center justify-between gap-4">
            <ViperTradeLogo size="md" />
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" asChild>
                <a href="/">Dashboard</a>
              </Button>
              <Button variant="outline" size="sm" onClick={fetchAnalysis}>
                Refresh
              </Button>
            </div>
          </div>
        </div>
      </header>

      <main className="container mx-auto px-4 py-4 space-y-4">
        <Card className="bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50">
          <CardHeader className="pb-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <CardTitle className="text-base text-slate-200">Analysis Overview</CardTitle>
              <div className="flex items-center gap-2">
                {analyst?.lookback_hours ? (
                  <Badge className="border-slate-600/70 bg-slate-900/60 text-[10px] tracking-[0.16em] text-slate-300">
                    {analyst.lookback_hours}H WINDOW
                  </Badge>
                ) : null}
                {analyst?.generated_at ? (
                  <div className="text-[11px] text-slate-500">
                    {new Date(analyst.generated_at).toLocaleString()}
                  </div>
                ) : null}
              </div>
            </div>
          </CardHeader>
          <CardContent className="pt-0 space-y-4">
            {loading && !payload ? (
              <div className="text-sm text-slate-400">Loading analysis...</div>
            ) : null}
            {error ? (
              <div className="text-sm text-red-300">Failed to load analysis: {error}</div>
            ) : null}
            {payload?.warnings?.length ? (
              <div className="rounded-xl border border-amber-500/25 bg-amber-500/10 px-4 py-3 text-sm text-amber-200">
                {payload.warnings.join(' · ')}
              </div>
            ) : null}

            <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Current Window</div>
                <div className="mt-3 text-3xl font-semibold tracking-[-0.03em] text-slate-100">
                  {analyst?.summary?.closed_trades ?? 0}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  {usd(analyst?.summary?.total_pnl_usdt)} · {num(analyst?.summary?.win_rate_pct)}% win rate
                </div>
              </div>

              <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Expectancy</div>
                <div
                  className={cn(
                    'mt-3 text-3xl font-semibold tracking-[-0.03em]',
                    (analyst?.expectancy?.expectancy_pct ?? 0) >= 0 ? 'text-emerald-300' : 'text-red-300',
                  )}
                >
                  {num(analyst?.expectancy?.expectancy_pct)}%
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  payoff {num(analyst?.expectancy?.payoff_ratio)} · {usd(analyst?.expectancy?.expectancy_usdt)} / trade
                </div>
              </div>

              <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Current Regime</div>
                  <Badge className={cn('text-[10px] tracking-[0.16em]', regimeTone.badge)}>
                    {regime?.status || 'mixed'}
                  </Badge>
                </div>
                <div className={cn('mt-3 text-2xl font-semibold tracking-[-0.03em]', regimeTone.text)}>
                  {titleCase((regime?.regime || 'balanced_mixed').replaceAll('_', ' '))}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  {titleCase((regime?.directional_bias || 'neutral').replaceAll('_', ' '))} · {titleCase(regime?.exit_profile || 'balanced')}
                </div>
              </div>

              <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Execution Advice</div>
                  <Badge className={cn('text-[10px] tracking-[0.16em]', executionTone.badge)}>
                    {titleCase(executionAdvice?.market_state || 'observation_mode')}
                  </Badge>
                </div>
                <div className="mt-3 text-2xl font-semibold tracking-[-0.03em] text-slate-100">
                  {titleCase((executionAdvice?.entry_action || 'only_best_setups').replaceAll('_', ' '))}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  exit {titleCase((executionAdvice?.exit_action || 'monitor_positions_closely').replaceAll('_', ' '))} · size {titleCase((executionAdvice?.size_action || 'reduced_size').replaceAll('_', ' '))}
                </div>
              </div>
            </div>

            <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 px-4 py-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">What Matters Now</div>
              <div className="mt-2 text-sm leading-6 text-slate-300">
                {executionAdvice?.summary || analyst?.llm_summary || analyst?.heuristic_summary || 'No analyst summary available yet.'}
              </div>
              {analyst?.tupa_error ? (
                <div className="mt-3 text-xs text-red-300">Tupa evaluation fallback: {analyst.tupa_error}</div>
              ) : null}
            </div>
          </CardContent>
        </Card>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
          <Card className="bg-panel/50 border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Focus</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                <div className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                  <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Priority Actions</div>
                  <div className="mt-3 space-y-2">
                    {(executionAdvice?.priority_actions || []).slice(0, 3).map((item) => (
                      <div key={item} className="rounded-lg border border-slate-700/40 bg-slate-950/50 px-3 py-2 text-sm text-slate-300">
                        {item}
                      </div>
                    ))}
                  </div>
                </div>

                <div className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                  <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Top Recommendations</div>
                  <div className="mt-3 space-y-2">
                    {(analyst?.recommendations || []).slice(0, 3).map((item) => {
                      const tone = toneClasses(item.severity);
                      return (
                        <div key={item.recommendation_id} className="rounded-lg border border-slate-700/40 bg-slate-950/50 px-3 py-2">
                          <div className="flex items-center justify-between gap-3">
                            <div className="text-sm font-medium text-slate-100">
                              {titleCase((item.recommendation || 'observe_more_sample').replaceAll('_', ' '))}
                            </div>
                            <Badge className={cn('text-[10px] tracking-[0.16em]', tone.badge)}>
                              {item.confidence || item.severity || 'info'}
                            </Badge>
                          </div>
                          <div className="mt-1 text-xs text-slate-500">{item.evidence}</div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card className="bg-panel/50 border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Performance Snapshot</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                <div className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                  <div className="flex items-center justify-between gap-3">
                    <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Comparative Status</div>
                    <Badge className={cn('text-[10px] tracking-[0.16em]', comparativeToneState.badge)}>
                      {comparative?.status || 'stable'}
                    </Badge>
                  </div>
                  <div className={cn('mt-3 text-lg font-semibold', comparativeToneState.text)}>
                    {titleCase((comparative?.status || 'stable').replaceAll('_', ' '))}
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {(comparative?.reasons || []).map((reason) => (
                      <Badge key={reason} className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300">
                        {titleCase(reason.replaceAll('_', ' '))}
                      </Badge>
                    ))}
                  </div>
                </div>

                <div className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                  <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Core Metrics</div>
                  <div className="mt-3 grid grid-cols-2 gap-3 text-xs text-slate-400">
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Thesis</div>
                      <div className={cn('mt-1', exitTone.text)}>
                        {num(analyst?.tupa_evaluation?.exit_pressure?.thesis_invalidated_pct)}%
                      </div>
                    </div>
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Trailing</div>
                      <div className="mt-1 text-slate-200">
                        {num(analyst?.tupa_evaluation?.exit_pressure?.trailing_stop_pct)}%
                      </div>
                    </div>
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Entry Gate</div>
                      <div className="mt-1 text-slate-200">{titleCase(analyst?.tupa_evaluation?.entry_pressure?.dominant_gate || 'unknown')}</div>
                    </div>
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Risk Symbol</div>
                      <div className={cn('mt-1', symbolTone.text)}>
                        {analyst?.tupa_evaluation?.symbol_risk?.symbol || 'Stable'}
                      </div>
                    </div>
                  </div>
                </div>

                <div className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                  <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Change Vs Previous Window</div>
                  <div className="mt-3 grid grid-cols-2 gap-3 text-xs text-slate-400">
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Expectancy Δ</div>
                      <div className="mt-1 text-slate-200">{num(comparative?.expectancy_pct?.delta)}%</div>
                    </div>
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Thesis Δ</div>
                      <div className="mt-1 text-slate-200">{num(comparative?.thesis_invalidated_pct?.delta)}%</div>
                    </div>
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Trailing Δ</div>
                      <div className="mt-1 text-slate-200">{num(comparative?.trailing_stop_pct?.delta)}%</div>
                    </div>
                    <div>
                      <div className="uppercase tracking-[0.16em] text-slate-500">Short Avg Δ</div>
                      <div className="mt-1 text-slate-200">{num(comparative?.short_avg_pnl_pct?.delta)}%</div>
                    </div>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
          <Card className="bg-panel/50 border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Watchlist</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                <div className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                  <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Fragile Symbols</div>
                  <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2">
                    {(analyst?.symbol_diagnostics || []).slice(0, 4).map((item) => {
                      const tone = comparativeTone(item.status);
                      return (
                        <div key={item.symbol} className="rounded-lg border border-slate-700/40 bg-slate-950/50 p-3">
                          <div className="flex items-center justify-between gap-3">
                            <div className="text-sm font-semibold text-slate-100">{item.symbol}</div>
                            <Badge className={cn('text-[10px] tracking-[0.16em]', tone.badge)}>
                              {item.status || 'mixed'}
                            </Badge>
                          </div>
                          <div className={cn('mt-2 text-sm font-medium', (item.avg_pnl_pct ?? 0) >= 0 ? 'text-emerald-300' : 'text-red-300')}>
                            {num(item.avg_pnl_pct)}%
                          </div>
                          <div className="mt-1 text-xs text-slate-500">
                            {titleCase((item.recommendation || 'observe_more_sample').replaceAll('_', ' '))}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>

                <div className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                  <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Current Hypothesis</div>
                  {(analyst?.hypotheses || []).slice(0, 1).map((item) => (
                    <div key={item.hypothesis_id} className="mt-3 space-y-2">
                      <div className="text-sm font-semibold text-slate-100">{item.hypothesis}</div>
                      <div className="text-xs text-slate-400">{item.evidence}</div>
                      <div className="rounded-lg border border-slate-700/40 bg-slate-950/50 px-3 py-2 text-xs text-slate-300">
                        <span className="font-semibold text-slate-100">Observe:</span> {item.observe}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        <Card className="bg-panel/50 border-border">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Active Position Advice</CardTitle>
          </CardHeader>
          <CardContent>
            {(analyst?.active_position_advice || []).length === 0 ? (
              <div className="text-sm text-muted-foreground">No active positions right now.</div>
            ) : (
              <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                {(analyst?.active_position_advice || []).map((item) => {
                  const tone = comparativeTone(
                    item.action === 'exit_recommended'
                      ? 'regressed'
                      : item.action === 'hold'
                        ? 'improved'
                        : 'mixed',
                  );
                  return (
                    <div key={`${item.symbol}-${item.side}`} className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-4">
                      <div className="flex items-center justify-between gap-3">
                        <div>
                          <div className="text-sm font-semibold text-slate-100">{item.symbol}</div>
                          <div className="text-xs text-slate-500">{item.side} · {item.duration_minutes ?? 0} min open</div>
                        </div>
                        <Badge className={cn('text-[10px] tracking-[0.16em]', tone.badge)}>
                          {titleCase((item.action || 'hold').replaceAll('_', ' '))}
                        </Badge>
                      </div>
                      <div className={cn('mt-3 text-lg font-semibold', (item.pnl_pct_estimate ?? 0) >= 0 ? 'text-emerald-300' : 'text-red-300')}>
                        {num(item.pnl_pct_estimate)}%
                      </div>
                      <div className="mt-2 text-sm text-slate-300">
                        {item.summary}
                      </div>
                      {(item.risk_flags || []).length > 0 ? (
                        <div className="mt-3 flex flex-wrap gap-2">
                          {(item.risk_flags || []).slice(0, 4).map((flag) => (
                            <Badge key={flag} className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300">
                              {titleCase(flag.replaceAll('_', ' '))}
                            </Badge>
                          ))}
                        </div>
                      ) : null}
                      {(item.evidence || []).length > 0 ? (
                        <div className="mt-3 space-y-1 text-xs text-slate-500">
                          {(item.evidence || []).slice(0, 3).map((evidence) => (
                            <div key={evidence}>{evidence}</div>
                          ))}
                        </div>
                      ) : null}
                    </div>
                  );
                })}
              </div>
            )}
          </CardContent>
        </Card>

        <details className="rounded-2xl border border-slate-700/60 bg-slate-900/40 p-4">
          <summary className="cursor-pointer list-none text-sm font-semibold text-slate-100">
            Deep Details
          </summary>
          <div className="mt-4 space-y-4">
            <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
              <BreakdownTable title="By Exit Reason" items={analyst?.by_close_reason || []} nameLabel="Exit" />
              <BreakdownTable title="By Symbol" items={analyst?.by_symbol || []} nameLabel="Symbol" />
            </div>

            <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
              <Card className="bg-panel/50 border-border">
                <CardHeader className="pb-2">
                  <CardTitle className="text-base">Top Entry Blockers</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-3">
                    {(analyst?.top_entry_blockers || []).slice(0, 6).map((item) => (
                      <div key={item.reason} className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="text-sm font-semibold text-slate-100">{titleCase(item.reason)}</div>
                          <div className="text-sm text-amber-300">{item.total}</div>
                        </div>
                      </div>
                    ))}
                  </div>
                </CardContent>
              </Card>

              <Card className="bg-panel/50 border-border">
                <CardHeader className="pb-2">
                  <CardTitle className="text-base">Thesis Invalidation Reasons</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-3">
                    {(analyst?.thesis_invalidation_breakdown || []).slice(0, 6).map((item) => (
                      <div key={item.reason} className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-3">
                        <div className="flex items-center justify-between gap-3">
                          <div className="text-sm font-semibold text-slate-100 break-all">{item.reason}</div>
                          <div className="text-sm text-amber-300">{item.total}</div>
                        </div>
                      </div>
                    ))}
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        </details>
      </main>
    </div>
  );
}
