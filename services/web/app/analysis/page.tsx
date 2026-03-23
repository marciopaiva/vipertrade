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
  thesis_invalidated_pct?: number;
  trailing_stop_pct?: number;
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
  by_close_reason?: BreakdownItem[];
  by_side?: BreakdownItem[];
  by_symbol?: BreakdownItem[];
  top_entry_blockers?: BlockerItem[];
  tupa_evaluation?: {
    exit_pressure?: AnalystEvaluationSignal;
    directional_bias?: AnalystEvaluationSignal;
    entry_pressure?: AnalystEvaluationSignal;
    symbol_risk?: AnalystEvaluationSignal;
  };
};

type DashboardResponse = {
  ai_analyst?: AiAnalystData;
  warnings?: string[];
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
  const entryTone = toneClasses(analyst?.tupa_evaluation?.entry_pressure?.severity);
  const symbolTone = toneClasses(analyst?.tupa_evaluation?.symbol_risk?.severity);

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
              <CardTitle className="text-base text-slate-200">AI Analyst Deep View</CardTitle>
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
                <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Closed Trades</div>
                <div className="mt-3 text-3xl font-semibold tracking-[-0.03em] text-slate-100">
                  {analyst?.summary?.closed_trades ?? 0}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  {usd(analyst?.summary?.total_pnl_usdt)} · {num(analyst?.summary?.win_rate_pct)}% win rate
                </div>
              </div>

              <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Exit Pressure</div>
                  <Badge className={cn('text-[10px] tracking-[0.16em]', exitTone.badge)}>
                    {analyst?.tupa_evaluation?.exit_pressure?.severity || 'pass'}
                  </Badge>
                </div>
                <div className={cn('mt-3 text-3xl font-semibold tracking-[-0.03em]', exitTone.text)}>
                  {num(analyst?.tupa_evaluation?.exit_pressure?.thesis_invalidated_pct)}%
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  trailing {num(analyst?.tupa_evaluation?.exit_pressure?.trailing_stop_pct)}%
                </div>
              </div>

              <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Entry Pressure</div>
                  <Badge className={cn('text-[10px] tracking-[0.16em]', entryTone.badge)}>
                    {analyst?.tupa_evaluation?.entry_pressure?.severity || 'warn'}
                  </Badge>
                </div>
                <div className="mt-3 text-3xl font-semibold tracking-[-0.03em] text-slate-100">
                  {titleCase(analyst?.tupa_evaluation?.entry_pressure?.dominant_gate || 'unknown')}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  {titleCase((analyst?.tupa_evaluation?.entry_pressure?.reason || '').replace('entry_pressure_', ''))}
                </div>
              </div>

              <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Symbol Risk</div>
                  <Badge className={cn('text-[10px] tracking-[0.16em]', symbolTone.badge)}>
                    {analyst?.tupa_evaluation?.symbol_risk?.severity || 'pass'}
                  </Badge>
                </div>
                <div className={cn('mt-3 text-3xl font-semibold tracking-[-0.03em]', symbolTone.text)}>
                  {analyst?.tupa_evaluation?.symbol_risk?.symbol || 'Stable'}
                </div>
                <div className="mt-2 text-xs text-slate-500">
                  {titleCase(analyst?.tupa_evaluation?.directional_bias?.reason?.replace('directional_bias_', '') || 'neutral')} bias
                </div>
              </div>
            </div>

            <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 px-4 py-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Summary</div>
              <div className="mt-2 text-sm leading-6 text-slate-300">
                {analyst?.llm_summary || analyst?.heuristic_summary || 'No analyst summary available yet.'}
              </div>
              {analyst?.tupa_error ? (
                <div className="mt-3 text-xs text-red-300">Tupa evaluation fallback: {analyst.tupa_error}</div>
              ) : null}
            </div>
          </CardContent>
        </Card>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
          <BreakdownTable title="By Exit Reason" items={analyst?.by_close_reason || []} nameLabel="Exit" />
          <BreakdownTable title="By Side" items={analyst?.by_side || []} nameLabel="Side" />
        </div>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
          <BreakdownTable title="By Symbol" items={analyst?.by_symbol || []} nameLabel="Symbol" />

          <Card className="bg-panel/50 border-border">
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Top Entry Blockers</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                {(analyst?.top_entry_blockers || []).length === 0 ? (
                  <div className="text-sm text-muted-foreground">No blockers captured.</div>
                ) : (
                  (analyst?.top_entry_blockers || []).map((item) => (
                    <div key={item.reason} className="rounded-xl border border-slate-700/60 bg-slate-900/50 p-3">
                      <div className="flex items-center justify-between gap-3">
                        <div className="text-sm font-semibold text-slate-100">{titleCase(item.reason)}</div>
                        <div className="text-sm text-amber-300">{item.total}</div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </CardContent>
          </Card>
        </div>
      </main>
    </div>
  );
}
