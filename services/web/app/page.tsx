'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useDashboard } from '@/hooks/useDashboard';
import { ViperTradeLogo } from '@/components/ViperTradeLogo';
import ServiceFlowDiagram from '@/components/dashboard/ServiceFlowDiagram';
import { PositionTable } from '@/components/dashboard/PositionTable';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

// Types
interface MarketSignal {
  symbol: string;
  current_price: number;
  atr_14: number;
  volume_24h: number;
  funding_rate: number;
  trend_score: number;
  spread_pct: number;
  regime?: string;
  consensus_side?: string;
  consensus_count?: number;
  exchanges_available?: number;
  bybit_regime?: string;
}

interface TokenDecision {
  symbol: string;
  regime: string;
  consensusSide: string;
  consensusLabel: string;
  bybitRegime: string;
  consensusCount: number;
  exchangesAvailable: number;
  trendScore: number;
  stateLabel: string;
  stateTone: 'positive' | 'negative' | 'neutral';
  stateContext?: string;
  bybitAligned: boolean;
  hasDivergence: boolean;
}

interface FlowContext {
  strategySymbol?: string;
  strategyState?: string;
  strategyContext?: string;
  executorSymbol?: string;
  executorAction?: string;
  executorContext?: string;
}

interface AnalystEvaluationSignal {
  reason?: string;
  severity?: 'pass' | 'warn' | 'fail';
  dominant_gate?: string;
  symbol?: string;
  recommendation?: string;
  top_reason?: string;
  thesis_invalidated_pct?: number;
  trailing_stop_pct?: number;
}

interface AiAnalystData {
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
  tupa_evaluation?: {
    exit_pressure?: AnalystEvaluationSignal;
    directional_bias?: AnalystEvaluationSignal;
    entry_pressure?: AnalystEvaluationSignal;
    thesis_quality?: AnalystEvaluationSignal;
    symbol_risk?: AnalystEvaluationSignal;
    summary?: {
      closed_trades?: number;
      total_pnl_usdt?: number;
      avg_pnl_pct?: number;
      avg_duration_s?: number;
      win_rate_pct?: number;
    };
  };
}

interface DashboardData {
  status: {
    trading_mode: string;
    trading_profile: string;
    trade_profile_label?: string;
    risk_status: string;
    db_connected: boolean;
    executor: { enabled: boolean; reason?: string };
    kill_switch: { enabled: boolean };
    risk_limits: {
      max_daily_loss_pct: number;
      max_leverage: number;
      risk_per_trade_pct: number;
    };
  };
  performance: {
    last_24h?: { total_trades: number; total_pnl: number; win_rate: number };
    last_7d?: { total_trades: number; total_pnl: number; win_rate: number };
  };
  positions: { items: Array<{
    trade_id: string;
    symbol: string;
    side: string;
    quantity: number;
    notional_usdt: number;
    entry_price: number;
    opened_at?: string;
    stop_loss_price?: number;
    trailing_activation_price?: number;
    fixed_take_profit_price?: number;
    break_even_price?: number;
    trailing_stop_activated?: boolean;
    trailing_stop_peak_price?: number;
    trailing_stop_final_distance_pct?: number;
  }> };
  trades: { items: Array<{
    trade_id: string;
    symbol: string;
    side: string;
    status: string;
    quantity: number;
    entry_price: number;
    exit_price?: number;
    pnl?: number;
    pnl_pct?: number;
    close_reason?: string;
    opened_at: string;
    closed_at?: string;
    duration_seconds?: number;
  }> };
  daily_trades_summary?: { count?: number };
  wallet?: {
    total_equity?: number;
    wallet_balance?: number;
    margin_balance?: number;
    available_balance?: number;
    unrealized_pnl?: number;
    initial_margin?: number;
    maintenance_margin?: number;
    account_im_rate?: number;
    account_mm_rate?: number;
    account_type?: string;
  };
  services: Array<{ name: string; ok: boolean; latency_ms: number }>;
  events?: { items?: Array<{ event_id: string; event_type: string; severity: string; timestamp: string; symbol?: string; data?: any }> };
  market_signals?: { items?: any[] | Record<string, any> };
  ai_analyst?: AiAnalystData;
  partial?: boolean;
  warnings?: string[];
}

// Helper functions
function num(value: number | null | undefined, digits = 2) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return value.toFixed(digits);
}

function usd(value: number | null | undefined) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 2 }).format(value);
}

function pct(value: number | null | undefined) {
  if (typeof value !== 'number' || Number.isNaN(value)) return '-';
  return `${(value * 100).toFixed(2)}%`;
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

// Components
function WalletCard({ label, amount, rate, accent = '#11c4ff' }: { label: string; amount?: number; rate?: number; accent?: string }) {
  return (
    <Card className="bg-panel/50 border-border">
      <CardContent className="pt-6">
        <div className="text-xs uppercase tracking-wider text-muted-foreground mb-2">{label}</div>
        <div className="text-2xl font-bold" style={{ color: accent }}>
          {usd(amount)}
        </div>
        {rate !== undefined && (
          <div className="text-xs text-muted-foreground mt-1">
            {pct(rate)}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function MetricCard({ label, value, accent = '#11c4ff', helper }: { label: string; value: string | number; accent?: string; helper?: string }) {
  return (
    <Card className="bg-panel/50 border-border">
      <CardContent className="pt-6">
        <div className="text-xs uppercase tracking-wider text-muted-foreground mb-2">{label}</div>
        <div className="text-2xl font-bold" style={{ color: accent }}>
          {value}
        </div>
        {helper && <div className="text-xs text-muted-foreground mt-1">{helper}</div>}
      </CardContent>
    </Card>
  );
}

function AiAnalystCard({ analyst }: { analyst?: AiAnalystData }) {
  const exitPressure = analyst?.tupa_evaluation?.exit_pressure;
  const entryPressure = analyst?.tupa_evaluation?.entry_pressure;
  const directionalBias = analyst?.tupa_evaluation?.directional_bias;
  const thesisQuality = analyst?.tupa_evaluation?.thesis_quality;
  const symbolRisk = analyst?.tupa_evaluation?.symbol_risk;
  const summary = analyst?.tupa_evaluation?.summary || analyst?.summary;
  const expectancy = analyst?.expectancy;
  const comparative = analyst?.comparative_diagnostics;
  const recommendations = analyst?.recommendations || [];
  const hypotheses = analyst?.hypotheses || [];
  const symbolDiagnostics = analyst?.symbol_diagnostics || [];
  const regime = analyst?.regime_diagnostics;
  const exitTone = toneClasses(exitPressure?.severity);
  const entryTone = toneClasses(entryPressure?.severity);
  const thesisTone = toneClasses(thesisQuality?.severity);
  const symbolTone = toneClasses(symbolRisk?.severity);
  const comparativeToneState = comparativeTone(comparative?.status);
  const regimeTone = comparativeTone(regime?.status);

  return (
    <Card className="bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50">
      <CardHeader className="pb-1">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="text-base text-slate-200">AI Analyst</CardTitle>
          <div className="flex items-center gap-2">
            {analyst?.lookback_hours ? (
              <Badge className="border-slate-600/70 bg-slate-900/60 text-[10px] tracking-[0.16em] text-slate-300">
                {analyst.lookback_hours}H WINDOW
              </Badge>
            ) : null}
            {analyst?.generated_at ? (
              <div className="text-[11px] text-slate-500">
                {new Date(analyst.generated_at).toLocaleTimeString()}
              </div>
            ) : null}
          </div>
        </div>
      </CardHeader>
      <CardContent className="pt-0 space-y-4">
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-6">
          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
            <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Directional Bias</div>
            <div className={cn('mt-3 text-2xl font-semibold tracking-[-0.03em]', toneClasses('pass').text)}>
              {titleCase(directionalBias?.reason?.replace('directional_bias_', '') || 'neutral')}
            </div>
            <div className="mt-2 text-xs text-slate-500">
              {summary?.closed_trades ?? 0} trades · {usd(summary?.total_pnl_usdt)}
            </div>
          </div>

          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Exit Pressure</div>
              <Badge className={cn('text-[10px] tracking-[0.16em]', exitTone.badge)}>
                {exitPressure?.severity || 'pass'}
              </Badge>
            </div>
            <div className={cn('mt-3 text-2xl font-semibold tracking-[-0.03em]', exitTone.text)}>
              {num(exitPressure?.thesis_invalidated_pct)}%
            </div>
            <div className="mt-2 text-xs text-slate-500">
              thesis invalidated · trailing {num(exitPressure?.trailing_stop_pct)}%
            </div>
          </div>

          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Entry Pressure</div>
              <Badge className={cn('text-[10px] tracking-[0.16em]', entryTone.badge)}>
                {entryPressure?.severity || 'warn'}
              </Badge>
            </div>
            <div className="mt-3 text-2xl font-semibold tracking-[-0.03em] text-slate-100">
              {titleCase(entryPressure?.dominant_gate || 'unknown')}
            </div>
            <div className="mt-2 text-xs text-slate-500">
              {titleCase((entryPressure?.reason || '').replace('entry_pressure_', '')) || 'No dominant gate'}
            </div>
          </div>

          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Thesis Guard</div>
              <Badge className={cn('text-[10px] tracking-[0.16em]', thesisTone.badge)}>
                {thesisQuality?.severity || 'pass'}
              </Badge>
            </div>
            <div className={cn('mt-3 text-xl font-semibold tracking-[-0.03em]', thesisTone.text)}>
              {titleCase((thesisQuality?.reason || 'stable').replace('thesis_quality_', ''))}
            </div>
            <div className="mt-2 text-xs text-slate-500">
              {titleCase((thesisQuality?.recommendation || 'keep_current_thesis_policy').replaceAll('_', ' '))}
            </div>
          </div>

          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
            <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Expectancy</div>
            <div
              className={cn(
                'mt-3 text-2xl font-semibold tracking-[-0.03em]',
                (expectancy?.expectancy_pct ?? 0) >= 0 ? 'text-emerald-300' : 'text-red-300',
              )}
            >
              {num(expectancy?.expectancy_pct)}%
            </div>
            <div className="mt-2 text-xs text-slate-500">
              payoff {num(expectancy?.payoff_ratio)} · {usd(expectancy?.expectancy_usdt)} / trade
            </div>
          </div>

          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Symbol Risk</div>
              <Badge className={cn('text-[10px] tracking-[0.16em]', symbolTone.badge)}>
                {symbolRisk?.severity || 'pass'}
              </Badge>
            </div>
            <div className={cn('mt-3 text-2xl font-semibold tracking-[-0.03em]', symbolTone.text)}>
              {symbolRisk?.symbol || 'Stable'}
            </div>
            <div className="mt-2 text-xs text-slate-500">
              {titleCase((symbolRisk?.reason || '').replace('symbol_risk_', '')) || 'No elevated symbol risk'}
            </div>
          </div>
        </div>

        <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 px-4 py-3">
          <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Analyst Summary</div>
          <div className="mt-2 text-sm leading-6 text-slate-300">
            {analyst?.llm_summary || analyst?.heuristic_summary || 'AI analyst summary unavailable.'}
          </div>
          {analyst?.tupa_error ? (
            <div className="mt-3 text-xs text-red-300">Tupa evaluation fallback: {analyst.tupa_error}</div>
          ) : null}
        </div>

        <div className="grid grid-cols-1 gap-3 xl:grid-cols-3">
          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">What Changed</div>
              <Badge className={cn('text-[10px] tracking-[0.16em]', comparativeToneState.badge)}>
                {comparative?.status || 'stable'}
              </Badge>
            </div>
            <div className={cn('mt-3 text-lg font-semibold', comparativeToneState.text)}>
              {titleCase((comparative?.status || 'stable').replaceAll('_', ' '))}
            </div>
            <div className="mt-2 text-xs text-slate-500">
              expectancy Δ {num(comparative?.expectancy_pct?.delta)}% · thesis Δ {num(comparative?.thesis_invalidated_pct?.delta)}%
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              {(comparative?.reasons || []).slice(0, 3).map((reason) => (
                <Badge
                  key={reason}
                  className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300"
                >
                  {titleCase(reason.replaceAll('_', ' '))}
                </Badge>
              ))}
            </div>
          </div>

          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Current Regime</div>
              <Badge className={cn('text-[10px] tracking-[0.16em]', regimeTone.badge)}>
                {regime?.status || 'mixed'}
              </Badge>
            </div>
            <div className={cn('mt-3 text-lg font-semibold', regimeTone.text)}>
              {titleCase((regime?.regime || 'balanced_mixed').replaceAll('_', ' '))}
            </div>
            <div className="mt-2 text-xs text-slate-500">
              {titleCase((regime?.directional_bias || 'neutral').replaceAll('_', ' '))} · {titleCase(regime?.exit_profile || 'balanced')}
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <Badge className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300">
                gate {titleCase(regime?.dominant_gate || 'unknown')}
              </Badge>
              <Badge className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300">
                {regime?.confidence || 'low'} confidence
              </Badge>
            </div>
          </div>

          <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 p-4">
            <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Top Recommendations</div>
            <div className="mt-3 space-y-3">
              {recommendations.length === 0 ? (
                <div className="text-sm text-slate-400">No recommendations yet.</div>
              ) : (
                recommendations.slice(0, 3).map((item) => {
                  const tone = toneClasses(item.severity);
                  return (
                    <div key={item.recommendation_id} className="rounded-xl border border-slate-700/60 bg-slate-950/60 p-3">
                      <div className="flex items-center justify-between gap-3">
                        <div className="text-sm font-semibold text-slate-100">
                          {titleCase((item.recommendation || 'observe').replaceAll('_', ' '))}
                        </div>
                        <Badge className={cn('text-[10px] tracking-[0.16em]', tone.badge)}>
                          {item.confidence || item.severity || 'info'}
                        </Badge>
                      </div>
                      <div className="mt-2 text-xs text-slate-400">{item.evidence}</div>
                    </div>
                  );
                })
              )}
            </div>
          </div>
        </div>

        <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 p-4">
          <div className="flex items-center justify-between gap-3">
            <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Hypotheses</div>
            <Badge className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300">
              {hypotheses.length} active
            </Badge>
          </div>
          <div className="mt-3 grid grid-cols-1 gap-3 xl:grid-cols-2">
            {hypotheses.length === 0 ? (
              <div className="text-sm text-slate-400">No testable hypotheses yet.</div>
            ) : (
              hypotheses.slice(0, 2).map((item) => (
                <div key={item.hypothesis_id} className="rounded-xl border border-slate-700/60 bg-slate-950/60 p-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="text-sm font-semibold text-slate-100">
                      {item.hypothesis || 'Observe more sample'}
                    </div>
                    <div className="flex items-center gap-2">
                      <Badge className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300">
                        {item.priority || 'info'}
                      </Badge>
                      <Badge className="border-slate-600/70 bg-slate-900/50 text-[10px] tracking-[0.12em] text-slate-300">
                        {item.confidence || 'low'}
                      </Badge>
                    </div>
                  </div>
                  <div className="mt-2 text-xs text-slate-400">{item.evidence}</div>
                  <div className="mt-3 text-xs text-slate-300">
                    <span className="font-semibold text-slate-100">Observe:</span> {item.observe}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>

        <div className="rounded-[20px] border border-slate-700/60 bg-slate-900/60 p-4">
          <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Symbols To Watch</div>
          <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
            {symbolDiagnostics.length === 0 ? (
              <div className="text-sm text-slate-400">No symbol diagnostics yet.</div>
            ) : (
              symbolDiagnostics.slice(0, 4).map((item) => {
                const tone = comparativeTone(item.status);
                return (
                  <div key={item.symbol} className="rounded-xl border border-slate-700/60 bg-slate-950/60 p-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className="text-sm font-semibold text-slate-100">{item.symbol || 'N/A'}</div>
                      <Badge className={cn('text-[10px] tracking-[0.16em]', tone.badge)}>
                        {item.status || 'mixed'}
                      </Badge>
                    </div>
                    <div className={cn('mt-2 text-sm font-medium', (item.avg_pnl_pct ?? 0) >= 0 ? 'text-emerald-300' : 'text-red-300')}>
                      {num(item.avg_pnl_pct)}%
                    </div>
                    <div className="mt-2 text-xs text-slate-400">
                      thesis {item.thesis_invalidated_trades ?? 0} · trailing {item.trailing_stop_trades ?? 0}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function ServicesGrid({ services }: { services: Array<{ name: string; ok: boolean; latency_ms: number }> }) {
  const flowOrder = ['bybit', 'market-data', 'strategy', 'executor', 'api', 'monitor', 'analytics', 'backtest'];
  const sortedServices = [...services].sort((a, b) => {
    const aIndex = flowOrder.findIndex(f => a.name.includes(f));
    const bIndex = flowOrder.findIndex(f => b.name.includes(f));
    return (aIndex === -1 ? 99 : aIndex) - (bIndex === -1 ? 99 : bIndex);
  });

  return (
    <Card className="bg-panel/50 border-border">
      <CardHeader>
        <CardTitle className="text-lg">Services Flow</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-3">
          {sortedServices.map((service) => (
            <div
              key={service.name}
              className={cn(
                'p-3 rounded-lg border text-center',
                service.ok 
                  ? 'border-green-500/30 bg-green-500/10' 
                  : 'border-red-500/30 bg-red-500/10'
              )}
            >
              <div className="text-xs text-muted-foreground capitalize truncate">{service.name}</div>
              <div className={cn('text-sm font-semibold mt-1', service.ok ? 'text-green-400' : 'text-red-400')}>
                {service.ok ? '✓' : '✗'}
              </div>
              {service.latency_ms > 0 && (
                <div className="text-xs text-muted-foreground mt-1">{service.latency_ms}ms</div>
              )}
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function ClosedTradesTable({ trades }: { trades: Array<{
  trade_id: string;
  symbol: string;
  side: string;
  status: string;
  quantity: number;
  entry_price: number;
  exit_price?: number;
  pnl?: number;
  pnl_pct?: number;
  close_reason?: string;
  opened_at: string;
  closed_at?: string;
  duration_seconds?: number;
}> }) {
  const [selectedDay, setSelectedDay] = useState<string>("");
  const [page, setPage] = useState(0);
  const pageSize = 10;

  // Filter closed trades from last 7 days
  const closedTrades = useMemo(() => {
    const sevenDaysAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
    return trades
      .filter(t => {
        if (t.status !== 'closed') return false;
        const refTime = Date.parse(t.closed_at || t.opened_at);
        return Number.isFinite(refTime) && refTime >= sevenDaysAgo;
      })
      .sort((a, b) => {
        const timeA = Date.parse(b.closed_at || b.opened_at);
        const timeB = Date.parse(a.closed_at || a.opened_at);
        return timeA - timeB;
      });
  }, [trades]);

  // Group by day
  const tradesByDay = useMemo(() => {
    const groups = new Map<string, { key: string; label: string; items: typeof closedTrades; latestTs: number }>();
    
    closedTrades.forEach(trade => {
      const refTime = Date.parse(trade.closed_at || trade.opened_at);
      if (!Number.isFinite(refTime)) return;
      
      const closedAt = new Date(refTime);
      const key = closedAt.toISOString().slice(0, 10);
      const label = closedAt.toLocaleDateString();
      const current = groups.get(key);
      
      if (current) {
        current.items.push(trade);
        current.latestTs = Math.max(current.latestTs, refTime);
      } else {
        groups.set(key, { key, label, items: [trade], latestTs: refTime });
      }
    });
    
    return Array.from(groups.values()).sort((a, b) => b.latestTs - a.latestTs);
  }, [closedTrades]);

  // Active day
  const activeDay = useMemo(() => {
    if (selectedDay && tradesByDay.some(g => g.key === selectedDay)) {
      return selectedDay;
    }
    return tradesByDay[0]?.key || "";
  }, [tradesByDay, selectedDay]);

  const activeGroup = useMemo(() => tradesByDay.find(g => g.key === activeDay) || null, [activeDay, tradesByDay]);
  const totalPages = useMemo(() => Math.max(1, Math.ceil((activeGroup?.items.length || 0) / pageSize)), [activeGroup]);
  const paginatedTrades = useMemo(() => {
    if (!activeGroup) return [];
    const start = page * pageSize;
    return activeGroup.items.slice(start, start + pageSize);
  }, [activeGroup, page]);

  // Reset page when day changes
  useEffect(() => {
    setPage(0);
  }, [activeDay]);

  if (closedTrades.length === 0) {
    return (
      <Card className="bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50">
        <CardHeader className="pb-2">
          <CardTitle className="text-lg text-slate-200">Recent Closed Trades</CardTitle>
        </CardHeader>
        <CardContent className="pt-0">
          <div className="text-center text-slate-500 py-8">No closed trades in the last 7 days</div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50">
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg text-slate-200">Recent Closed Trades</CardTitle>
          <Badge variant="outline" className="text-xs border-slate-600 text-slate-400">
            Last 7 days
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="pt-0">
        {/* Day selector - compact */}
        {tradesByDay.length > 1 && (
          <div className="flex gap-1 mb-3 flex-wrap">
            {tradesByDay.map(day => (
              <Button
                key={day.key}
                variant={activeDay === day.key ? 'default' : 'outline'}
                size="sm"
                onClick={() => setSelectedDay(day.key)}
                className="text-xs px-2 py-1 h-7"
              >
                {day.label.split(',')[0]} ({day.items.length})
              </Button>
            ))}
          </div>
        )}

        <div className="hidden xl:grid xl:grid-cols-[220px_70px_110px_110px_110px_120px_1fr] gap-4 px-3 pb-2 text-[11px] uppercase tracking-[0.18em] text-slate-500">
          <div>Asset</div>
          <div>Side</div>
          <div className="text-right">PnL</div>
          <div>Entry</div>
          <div>Exit</div>
          <div>Closed</div>
          <div>Reason</div>
        </div>

        {/* Trades list - compact */}
        <div className="space-y-2">
          {paginatedTrades.map(trade => {
            const pnl = trade.pnl || 0;
            const pnlColor = pnl >= 0 ? '#10b981' : '#ef4444';
            const sideColor = trade.side === 'Long' ? '#10b981' : '#ef4444';
            const durationLabel = trade.duration_seconds
              ? `${Math.max(1, Math.round(trade.duration_seconds / 60))}m`
              : '-';
            const reasonLabel = titleCase(trade.close_reason || 'unknown');
            const closedAt = trade.closed_at ? new Date(trade.closed_at) : null;
            const closedDateLabel = closedAt
              ? closedAt.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
              : '-';
            const closedTimeLabel = closedAt
              ? closedAt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
              : '-';

            return (
              <div
                key={trade.trade_id}
                className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-3"
              >
                <div className="grid grid-cols-1 gap-4 xl:grid-cols-[220px_70px_110px_110px_110px_120px_1fr] xl:items-center">
                  <div className="min-w-0">
                    <div className="text-sm font-bold text-slate-200">{trade.symbol}</div>
                  </div>

                  <div>
                    <Badge
                      style={{ backgroundColor: sideColor + '22', color: sideColor, borderColor: sideColor + '55' }}
                      className="h-5 min-w-[58px] justify-center px-1.5 py-0.5 text-[10px]"
                    >
                      {trade.side.toUpperCase()}
                    </Badge>
                  </div>

                  <div className="text-right xl:pr-2">
                    <div className="text-sm font-bold" style={{ color: pnlColor }}>
                      {pnl ? `$${pnl.toFixed(2)}` : '-'}
                    </div>
                    <div className="text-xs" style={{ color: pnlColor }}>
                      {trade.pnl_pct ? `${(trade.pnl_pct * 100).toFixed(2)}%` : '-'}
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-2 text-xs md:contents">
                    <div>
                      <Badge
                        className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                        style={{
                          backgroundColor: '#0f172acc',
                          color: '#e2e8f0',
                          borderColor: '#334155',
                        }}
                      >
                        ${trade.entry_price.toFixed(6)}
                      </Badge>
                    </div>
                    <div>
                      {trade.exit_price ? (
                        <Badge
                          className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                          style={{
                            backgroundColor: '#0f172acc',
                            color: '#e2e8f0',
                            borderColor: '#334155',
                          }}
                        >
                          ${trade.exit_price.toFixed(6)}
                        </Badge>
                      ) : (
                        <div className="flex h-6 items-center justify-center text-slate-300">-</div>
                      )}
                    </div>
                  </div>

                  <div>
                    <div className="text-xs text-slate-200">{closedDateLabel}</div>
                    <div className="text-[11px] text-slate-400">{closedTimeLabel}</div>
                  </div>

                  <div className="flex items-center gap-3 text-xs">
                    <span className="text-slate-200">{reasonLabel}</span>
                    {durationLabel !== '-' && (
                      <>
                        <span className="text-slate-600">·</span>
                        <span className="text-slate-400">{durationLabel}</span>
                      </>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>

        {/* Pagination - compact */}
        {activeGroup && activeGroup.items.length > pageSize && (
          <div className="flex items-center justify-between mt-3">
            <div className="text-xs text-slate-500">
              {activeGroup.label} · {activeGroup.items.length} trades · p.{page + 1}/{totalPages}
            </div>
            <div className="flex gap-1">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage(p => Math.max(0, p - 1))}
                disabled={page === 0}
                className="text-xs px-2 py-1 h-7"
              >
                Prev
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
                disabled={page >= totalPages - 1}
                className="text-xs px-2 py-1 h-7"
              >
                Next
              </Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default function DashboardPage() {
  const [refreshKey, setRefreshKey] = useState(0);
  const [dashboardData, setDashboardData] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastStableMarketSignals, setLastStableMarketSignals] = useState<Record<string, any>>({});

  // Fetch from Next.js API route directly (has market_signals)
  const fetchDashboard = useCallback(async () => {
    try {
      const res = await fetch('/api/dashboard', { cache: 'no-store' });
      const data = await res.json();
      setDashboardData(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshAll = useCallback(async () => {
    await fetchDashboard();
    setRefreshKey(k => k + 1);
  }, [fetchDashboard]);

  useEffect(() => {
    fetchDashboard();
    const interval = setInterval(fetchDashboard, 5000);
    return () => clearInterval(interval);
  }, [fetchDashboard]);

  const data = useMemo<DashboardData | null>(() => {
    if (!dashboardData) return null;
    return {
      status: dashboardData.status || dashboardData,
      performance: dashboardData.performance || {
        last_24h: { total_trades: 0, total_pnl: 0, win_rate: 0 },
        last_7d: { total_trades: 0, total_pnl: 0, win_rate: 0 },
      },
      positions: dashboardData.positions || { items: [] },
      trades: dashboardData.trades || { items: [] },
      wallet: dashboardData.wallet,
      services: dashboardData.services || [],
      events: dashboardData.events || { items: [] },
      market_signals: dashboardData.market_signals || { items: {} },
      ai_analyst: dashboardData.ai_analyst,
      partial: Boolean(dashboardData.partial),
      warnings: Array.isArray(dashboardData.warnings) ? dashboardData.warnings : [],
    };
  }, [dashboardData, refreshKey]);

  const liveMarketSignals = useMemo<Record<string, any>>(() => {
    const items = data?.market_signals?.items;
    if (!items) return {};
    if (Array.isArray(items)) {
      return Object.fromEntries(items.map((signal: any) => [signal.symbol, signal]));
    }
    return items as Record<string, any>;
  }, [data?.market_signals?.items]);

  useEffect(() => {
    if (Object.keys(liveMarketSignals).length > 0) {
      setLastStableMarketSignals(liveMarketSignals);
    }
  }, [liveMarketSignals]);

  const effectiveMarketSignals = useMemo<Record<string, any>>(() => {
    if (Object.keys(liveMarketSignals).length > 0) return liveMarketSignals;
    return lastStableMarketSignals;
  }, [lastStableMarketSignals, liveMarketSignals]);

  const decisionMatrixStale = Object.keys(liveMarketSignals).length === 0 && Object.keys(lastStableMarketSignals).length > 0;
  
  // Build Decision Matrix from market signals
  const tokenDecisions = useMemo<TokenDecision[]>(() => {
    const signalsObj = effectiveMarketSignals;
    const signalsArray = Object.values(signalsObj);
    if (signalsArray.length === 0) return [];

    const events = data?.events?.items || [];
    const positions = data?.positions?.items || [];

    const latestExecutorEventBySymbol = new Map<string, { action?: string; status?: string }>();
    for (const event of events) {
      if (event.event_type !== 'executor_event_processed' || !event.symbol || latestExecutorEventBySymbol.has(event.symbol)) {
        continue;
      }
      latestExecutorEventBySymbol.set(event.symbol, {
        action: event.data?.action ? String(event.data.action) : undefined,
        status: event.data?.status ? String(event.data.status) : undefined,
      });
    }
    
    return signalsArray.map((signal: any) => {
      const consensusSide = signal.consensus_side || signal.regime || 'neutral';
      const consensusCount = signal.consensus_count || 0;
      const exchanges = signal.exchanges_available || 0;
      const executorEvent = latestExecutorEventBySymbol.get(signal.symbol);
      const openPosition = positions.find((position) => position.symbol === signal.symbol);
      
      let stateLabel = 'Watching';
      let stateTone: 'positive' | 'negative' | 'neutral' = 'neutral';
      let stateContext: string | undefined;
      let consensusLabel = 'Mixed Consensus';
      
      if (consensusSide === 'bullish' && consensusCount >= 2) {
        consensusLabel = 'Bullish Consensus';
      } else if (consensusSide === 'bearish' && consensusCount >= 2) {
        consensusLabel = 'Bearish Consensus';
      } else if (consensusSide === 'bullish') {
        consensusLabel = 'Bullish Watch';
      } else if (consensusSide === 'bearish') {
        consensusLabel = 'Bearish Watch';
      }

      if (openPosition) {
        stateLabel = `Open ${String(openPosition.side)}`;
        stateTone = String(openPosition.side).toLowerCase() === 'long' ? 'positive' : 'negative';
        stateContext = `${usd(openPosition.notional_usdt)} live`;
      } else if (executorEvent?.status === 'paper_open') {
        const action = String(executorEvent.action || '').toUpperCase();
        stateLabel = action === 'ENTER_LONG' ? 'Enter Long' : action === 'ENTER_SHORT' ? 'Enter Short' : 'Opening';
        stateTone = action === 'ENTER_LONG' ? 'positive' : action === 'ENTER_SHORT' ? 'negative' : 'neutral';
        stateContext = 'Executor accepted';
      } else if (executorEvent?.status === 'ignored_hold') {
        stateLabel = 'Hold';
        stateTone = 'neutral';
        stateContext = 'Strategy blocked entry';
      } else if (executorEvent?.status?.startsWith('blocked_')) {
        stateLabel = 'Blocked';
        stateTone = 'neutral';
        stateContext = titleCase(executorEvent.status);
      }
      
      const bybitAligned = consensusSide !== 'neutral' && (signal.bybit_regime || 'neutral') === consensusSide;
      const hasDivergence = exchanges > 0 && consensusCount < exchanges;
      
      return {
        symbol: signal.symbol,
        regime: signal.regime || 'neutral',
        consensusSide,
        consensusLabel,
        bybitRegime: signal.bybit_regime || 'neutral',
        consensusCount,
        exchangesAvailable: exchanges,
        trendScore: signal.trend_score || 0,
        stateLabel,
        stateTone,
        stateContext,
        bybitAligned,
        hasDivergence,
      };
    }).sort((a: any, b: any) => {
      const tonePriority: Record<string, number> = { positive: 0, negative: 1, neutral: 2 };
      return tonePriority[a.stateTone] - tonePriority[b.stateTone] ||
             b.consensusCount - a.consensusCount ||
             Math.abs(b.trendScore) - Math.abs(a.trendScore);
    });
  }, [data?.events?.items, data?.positions?.items, effectiveMarketSignals]);

  const flowContext = useMemo<FlowContext>(() => {
    const signalsObj = effectiveMarketSignals;
    const latestExecutorEvent = (data?.events?.items || []).find(
      (event) => event.event_type === 'executor_event_processed' && event.symbol
    );
    const leadToken = latestExecutorEvent?.symbol
      ? tokenDecisions.find((token) => token.symbol === latestExecutorEvent.symbol) || tokenDecisions[0]
      : tokenDecisions[0];
    const leadSignal = leadToken ? signalsObj[leadToken.symbol] : null;

    const openPosition = (data?.positions?.items || [])[0];
    const lastClosedTrade = (data?.trades?.items || []).find((trade) => trade.status === 'closed');

    const strategyState = leadToken
      ? leadToken.stateLabel
      : 'scan idle';
    const strategyContext = leadSignal
      ? leadToken?.stateContext || `${leadSignal.consensus_count || 0}/${leadSignal.exchanges_available || 0} consensus`
      : undefined;

    const executorSymbol =
      openPosition?.symbol ||
      latestExecutorEvent?.symbol ||
      lastClosedTrade?.symbol;

    const executorAction = openPosition
      ? `open ${openPosition.side.toLowerCase()}`
      : latestExecutorEvent?.data?.action
        ? String(latestExecutorEvent.data.action).toLowerCase()
        : 'idle';

    const executorContext = openPosition
      ? `${usd(openPosition.notional_usdt)} live`
      : latestExecutorEvent?.data?.status
        ? String(latestExecutorEvent.data.status).replaceAll('_', ' ')
        : 'awaiting decision';

    return {
      strategySymbol: leadToken?.symbol,
      strategyState,
      strategyContext,
      executorSymbol,
      executorAction,
      executorContext,
    };
  }, [data?.events?.items, data?.positions?.items, data?.trades?.items, effectiveMarketSignals, tokenDecisions]);

  if (loading && !dashboardData) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <div className="text-2xl font-bold text-primary mb-2">Loading...</div>
          <div className="text-muted-foreground">Connecting to ViperTrade</div>
        </div>
      </div>
    );
  }

  const tradingMode = data?.status?.trading_mode?.toLowerCase() as 'paper' | 'testnet' | 'mainnet' || 'paper';
  const executorEnabled = data?.status?.executor?.enabled ?? false;

  return (
    <div className="min-h-screen bg-background">
      {/* Header */}
      <header className="border-b border-border bg-panel/50 backdrop-blur-sm sticky top-0 z-50">
        <div className="container mx-auto px-4 py-4">
          <div className="flex items-center justify-between">
            <ViperTradeLogo size="md" />
            <div className="flex items-center gap-2">
              <Button variant="outline" size="sm" asChild>
                <a href="/analysis">Analysis</a>
              </Button>
              <Button variant="outline" size="sm" onClick={refreshAll}>
                Refresh
              </Button>
            </div>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="container mx-auto px-4 py-4 space-y-4">
        {/* Architecture Flow */}
        <Card className="bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50">
          <CardHeader className="pb-1">
            <CardTitle className="text-base text-slate-200">Architecture Flow</CardTitle>
          </CardHeader>
          <CardContent className="pt-0">
            <ServiceFlowDiagram
              services={data?.services || []}
              executionMode={tradingMode}
              executorState={executorEnabled ? 'running' : 'down'}
              flowContext={flowContext}
              activeSignalsCount={Object.keys(effectiveMarketSignals).length}
              openPositionsCount={data?.positions?.items?.length || 0}
              closedTradesCount={(data?.trades?.items || []).filter((trade) => trade.status === 'closed').length}
            />
          </CardContent>
        </Card>

        <AiAnalystCard analyst={data?.ai_analyst} />

        {/* Wallet Card - Unified */}
        <Card className="bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50">
          <CardHeader className="pb-1">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base text-slate-200">Wallet Overview</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="pt-0 space-y-4">
            <div className="relative overflow-hidden rounded-[28px] border border-slate-700/60 bg-[radial-gradient(circle_at_top_right,rgba(16,185,129,0.16),transparent_28%),linear-gradient(180deg,rgba(15,23,42,0.74),rgba(15,23,42,0.42))] px-6 py-5">
              <div className="absolute right-4 top-4 hidden sm:block">
                <svg width="120" height="56" viewBox="0 0 120 56" className="opacity-80">
                  <defs>
                    <linearGradient id="walletLine" x1="0%" y1="0%" x2="100%" y2="0%">
                      <stop offset="0%" stopColor="#10b981" stopOpacity="0.2" />
                      <stop offset="100%" stopColor="#34d399" stopOpacity="0.95" />
                    </linearGradient>
                    <linearGradient id="walletFill" x1="0%" y1="0%" x2="0%" y2="100%">
                      <stop offset="0%" stopColor="#10b981" stopOpacity="0.28" />
                      <stop offset="100%" stopColor="#10b981" stopOpacity="0" />
                    </linearGradient>
                  </defs>
                  <path d="M10 42 L34 36 L58 30 L82 18 L110 6" fill="none" stroke="url(#walletLine)" strokeWidth="2.5" strokeLinecap="round" />
                  <path d="M10 42 L34 36 L58 30 L82 18 L110 6 L110 56 L10 56 Z" fill="url(#walletFill)" />
                </svg>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <div className="text-[11px] uppercase tracking-[0.32em] text-slate-500">Portfolio</div>
                <Badge className="border-emerald-500/40 bg-emerald-500/10 text-[10px] tracking-[0.18em] text-emerald-300">
                  Live
                </Badge>
              </div>

              <div className="mt-4 flex flex-wrap items-end gap-x-4 gap-y-3">
                <div className="text-5xl font-semibold tracking-[-0.04em] text-slate-50 sm:text-6xl">
                  {usd(data?.wallet?.total_equity)}
                </div>
                <div className={cn(
                  'rounded-full border px-3 py-1 text-sm font-semibold',
                  (data?.performance?.last_7d?.total_pnl ?? 0) >= 0
                    ? 'border-emerald-500/35 bg-emerald-500/10 text-emerald-300'
                    : 'border-red-500/35 bg-red-500/10 text-red-300'
                )}>
                  {usd(data?.performance?.last_7d?.total_pnl)} · 7d
                </div>
              </div>

              <div className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-2 text-sm">
                <div className="text-slate-500">
                  Profile{' '}
                  <span className="font-semibold text-slate-200">
                    {data?.status?.trade_profile_label || data?.status?.trading_profile || 'MEDIUM'}
                  </span>
                </div>
                <div className="text-slate-500">
                  Open <span className="font-semibold text-violet-300">{data?.positions?.items?.length || 0}</span>
                </div>
                <div className={cn('font-medium', (data?.wallet?.unrealized_pnl ?? 0) >= 0 ? 'text-emerald-300' : 'text-red-300')}>
                  {usd(data?.wallet?.unrealized_pnl)} unrealized
                </div>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="relative overflow-hidden rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Deposited</div>
                <div className="mt-3 text-[2rem] font-semibold tracking-[-0.03em] text-slate-100">{usd(data?.wallet?.wallet_balance)}</div>
                <div className="mt-2 text-xs text-slate-500">
                  {(data?.wallet?.margin_balance ?? 0) > 0
                    ? `${(((data?.wallet?.initial_margin || 0) / Math.max(1, data?.wallet?.margin_balance || 1)) * 100).toFixed(0)}% active`
                    : 'No active margin'}
                </div>
                <div className="absolute -right-5 -top-5 h-20 w-20 rounded-full border border-slate-700/70" />
              </div>

              <div className="relative overflow-hidden rounded-[20px] border border-slate-700/60 bg-slate-900/70 p-4">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Earnings</div>
                    <div className={cn('mt-3 text-[2rem] font-semibold tracking-[-0.03em]', (data?.performance?.last_24h?.total_pnl ?? 0) >= 0 ? 'text-emerald-300' : 'text-red-300')}>
                      {usd(data?.performance?.last_24h?.total_pnl)}
                    </div>
                    <div className="mt-2 text-xs text-slate-500">
                      {data?.performance?.last_24h?.win_rate !== undefined
                        ? `${data.performance.last_24h.win_rate.toFixed(1)}% win rate`
                        : '24h performance'}
                    </div>
                  </div>
                  <div className="text-xs text-slate-500">24h</div>
                </div>
                <div className="absolute bottom-0 right-0 h-14 w-20 rounded-tl-2xl bg-[linear-gradient(135deg,rgba(15,23,42,0)_0%,rgba(59,130,246,0.14)_100%)]" />
              </div>

              <div className="rounded-[20px] border border-emerald-500/20 bg-emerald-500/[0.08] p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Active Trading</div>
                <div className="mt-3 text-[2rem] font-semibold tracking-[-0.03em] text-slate-100">{usd(data?.wallet?.margin_balance)}</div>
                <div className="mt-2 flex items-center gap-2 text-xs text-emerald-300">
                  <span className="h-2 w-2 rounded-full bg-emerald-400" />
                  Working for you
                </div>
              </div>

              <div className="rounded-[20px] border border-amber-500/20 bg-amber-500/[0.06] p-4">
                <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Idle Funds</div>
                <div className="mt-3 text-[2rem] font-semibold tracking-[-0.03em] text-amber-300">{usd(data?.wallet?.available_balance)}</div>
                <div className="mt-2 text-xs text-amber-200/80">
                  Ready capital
                </div>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Decision Matrix */}
        <Card className="bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between gap-3">
              <CardTitle className="text-lg text-slate-200">Decision Matrix</CardTitle>
              {decisionMatrixStale && (
                <Badge className="border-amber-500/30 bg-amber-500/10 text-amber-300">
                  Refreshing signals
                </Badge>
              )}
            </div>
          </CardHeader>
          <CardContent className="pt-0">
            {tokenDecisions.length === 0 ? (
              <div className="text-center text-slate-500 py-8">
                {data?.partial ? 'Decision feed is temporarily unavailable' : 'No decision data available'}
              </div>
            ) : (
              <div className="space-y-3">
                <div className="grid grid-cols-1 gap-3 xl:grid-cols-[1.3fr_1fr_1fr]">
                  <div className="rounded-[22px] border border-slate-700/50 bg-[radial-gradient(circle_at_top_right,rgba(99,102,241,0.12),transparent_32%),linear-gradient(180deg,rgba(30,41,59,0.55),rgba(15,23,42,0.38))] px-4 py-3">
                    <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Strategy Focus</div>
                    <div className="mt-2 text-lg font-semibold tracking-[-0.02em] text-slate-50">
                      {flowContext.strategySymbol || 'No active setup'}
                    </div>
                    <div className="mt-1 text-sm text-slate-300">
                      {flowContext.strategyState || 'Scanning market conditions'}
                    </div>
                    <div className="mt-2 text-xs text-slate-500">{flowContext.strategyContext || 'No dominant block reason right now'}</div>
                  </div>
                  <div className="rounded-[22px] border border-slate-700/50 bg-slate-800/35 px-4 py-3">
                    <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Executor</div>
                    <div className="mt-2 text-lg font-semibold tracking-[-0.02em] text-slate-50">
                      {flowContext.executorSymbol || (executorEnabled ? 'Executor idle' : 'Executor paused')}
                    </div>
                    <div className="mt-1 text-sm text-slate-300">
                      {flowContext.executorAction || (executorEnabled ? 'Awaiting valid decision' : 'Stopped')}
                    </div>
                    <div className="mt-2 text-xs text-slate-500">{flowContext.executorContext || 'No execution pressure on the hot path'}</div>
                  </div>
                  <div className="grid grid-cols-3 gap-2">
                    <div className="rounded-[20px] border border-slate-700/50 bg-slate-800/30 px-3 py-3">
                      <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Actionable</div>
                      <div className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-emerald-300">
                        {tokenDecisions.filter((token) => token.stateTone === 'positive').length}
                      </div>
                    </div>
                    <div className="rounded-[20px] border border-slate-700/50 bg-slate-800/30 px-3 py-3">
                      <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Hold</div>
                      <div className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-violet-300">
                        {tokenDecisions.filter((token) => token.stateTone === 'neutral').length}
                      </div>
                    </div>
                    <div className="rounded-[20px] border border-slate-700/50 bg-slate-800/30 px-3 py-3">
                      <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Divergence</div>
                      <div className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-amber-300">
                        {tokenDecisions.filter((token) => token.hasDivergence).length}
                      </div>
                    </div>
                  </div>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
                  {tokenDecisions.map((token) => {
                  const stateColor = token.stateTone === 'positive' ? '#10b981'
                    : token.stateTone === 'negative' ? '#ef4444'
                    : '#6366f1';
                  const alignmentColor = token.bybitAligned ? '#10b981'
                    : token.hasDivergence ? '#f59e0b'
                    : '#64748b';
                  const trendPositive = token.trendScore >= 0;

                    return (
                      <div
                        key={token.symbol}
                        className="rounded-[22px] border border-slate-700/50 bg-[linear-gradient(180deg,rgba(30,41,59,0.58),rgba(15,23,42,0.38))] p-4 shadow-[0_12px_28px_rgba(2,6,23,0.18)]"
                      >
                        <div className="flex justify-between items-start gap-3">
                          <div>
                            <div className="text-lg font-semibold tracking-[-0.02em] text-slate-50">{token.symbol}</div>
                            <div className="mt-1 text-xs text-slate-500">
                              {token.consensusLabel}
                            </div>
                          </div>
                          <Badge
                            style={{ backgroundColor: stateColor + '22', color: stateColor, borderColor: stateColor + '55' }}
                            className="text-xs"
                          >
                            {token.stateLabel}
                          </Badge>
                        </div>

                        <div className="mt-5 flex items-end justify-between gap-3">
                          <div>
                            <div className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Trend</div>
                            <div className={cn('mt-1 text-3xl font-semibold tracking-[-0.04em]', trendPositive ? 'text-slate-50' : 'text-red-300')}>
                              {trendPositive ? '+' : ''}{token.trendScore.toFixed(3)}
                            </div>
                            <div className="mt-2 text-xs text-slate-500">
                              {token.stateContext || `${token.consensusCount}/${token.exchangesAvailable} exchanges aligned`}
                            </div>
                          </div>
                          <Badge
                            style={{ backgroundColor: alignmentColor + '22', color: alignmentColor, borderColor: alignmentColor + '55' }}
                            className="text-xs"
                          >
                            {token.bybitAligned ? 'Aligned' : token.hasDivergence ? 'Divergent' : 'Watching'}
                          </Badge>
                        </div>

                        <div className="mt-4 grid grid-cols-2 gap-2 border-t border-slate-700/50 pt-3">
                          <div className="rounded-2xl bg-slate-900/35 px-3 py-2">
                            <div className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Consensus</div>
                            <div className="mt-1 text-sm font-semibold text-slate-200 truncate">{token.consensusSide}</div>
                          </div>
                          <div className="rounded-2xl bg-slate-900/35 px-3 py-2">
                            <div className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Bybit</div>
                            <div className="mt-1 text-sm font-semibold text-slate-200 truncate">{token.bybitRegime}</div>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Positions */}
        <PositionTable 
          positions={data?.positions?.items || []} 
          marketSignals={data?.market_signals?.items ? Object.values(data.market_signals.items as any) : []}
        />

        {/* Closed Trades */}
        <ClosedTradesTable trades={data?.trades?.items || []} />
      </main>
    </div>
  );
}
