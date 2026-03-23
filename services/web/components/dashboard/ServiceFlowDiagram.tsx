'use client';

import React, { useMemo } from 'react';

interface ServiceFlowDiagramProps {
  services?: Array<{ name: string; ok: boolean; latency_ms: number }>;
  executionMode?: 'paper' | 'testnet' | 'mainnet';
  executorState?: 'running' | 'paused' | 'down';
  flowContext?: {
    strategySymbol?: string;
    strategyState?: string;
    strategyContext?: string;
    executorSymbol?: string;
    executorAction?: string;
    executorContext?: string;
  };
  activeSignalsCount?: number;
  openPositionsCount?: number;
  closedTradesCount?: number;
}

const STAGE_COLORS = {
  sources: {
    border: 'border-slate-700/70',
    panel: 'bg-slate-950/45',
    pill: 'border-slate-700/70 bg-slate-900/70 text-slate-200',
  },
  marketData: {
    border: 'border-sky-400/55',
    panel: 'bg-sky-500/[0.06]',
    glow: 'shadow-[0_0_0_1px_rgba(56,189,248,0.14),0_14px_30px_rgba(14,165,233,0.10)]',
    accent: 'text-sky-300',
    dot: 'bg-sky-400',
    rail: 'from-sky-400/35 via-sky-400/10 to-transparent',
  },
  strategy: {
    border: 'border-violet-400/55',
    panel: 'bg-violet-500/[0.06]',
    glow: 'shadow-[0_0_0_1px_rgba(192,132,252,0.14),0_14px_30px_rgba(139,92,246,0.10)]',
    accent: 'text-violet-300',
    dot: 'bg-violet-400',
    rail: 'from-violet-400/35 via-violet-400/10 to-transparent',
  },
  executor: {
    border: 'border-emerald-400/65',
    panel: 'bg-emerald-500/[0.07]',
    glow: 'shadow-[0_0_0_1px_rgba(52,211,153,0.18),0_0_38px_rgba(16,185,129,0.14)]',
    accent: 'text-emerald-300',
    dot: 'bg-emerald-400',
    rail: 'from-emerald-400/40 via-emerald-400/12 to-transparent',
  },
  sidecars: {
    border: 'border-slate-700/70',
    panel: 'bg-slate-950/45',
    pill: 'border-slate-700/70 bg-slate-900/60 text-slate-200',
  },
};

function normalizeName(name: string) {
  return name.toLowerCase();
}

function getService(services: Array<{ name: string; ok: boolean; latency_ms: number }>, matcher: string) {
  return services.find((svc) => normalizeName(svc.name).includes(matcher));
}

function getStatusTone(ok: boolean | undefined, latency: number | undefined) {
  if (ok === false) {
    return {
      ring: 'ring-1 ring-red-400/45',
      badge: 'text-red-300 border-red-400/40 bg-red-500/10',
      dot: 'bg-red-400',
      label: 'down',
    };
  }
  if ((latency ?? 0) > 500) {
    return {
      ring: 'ring-1 ring-amber-400/40',
      badge: 'text-amber-300 border-amber-400/35 bg-amber-500/10',
      dot: 'bg-amber-400',
      label: 'slow',
    };
  }
  return {
    ring: 'ring-1 ring-emerald-400/35',
    badge: 'text-emerald-300 border-emerald-400/30 bg-emerald-500/10',
    dot: 'bg-emerald-400',
    label: 'live',
  };
}

function StageCard({
  title,
  latency,
  subtitle,
  statusLine,
  detailLine,
  color,
  tone,
  hero = false,
  animated = false,
}: {
  title: string;
  latency?: number;
  subtitle: string;
  statusLine?: string;
  detailLine?: string;
  color: typeof STAGE_COLORS.marketData;
  tone: ReturnType<typeof getStatusTone>;
  hero?: boolean;
  animated?: boolean;
}) {
  return (
    <div
      className={[
        'relative overflow-hidden rounded-2xl border backdrop-blur-sm',
        hero ? 'min-h-[128px] px-4 py-4' : 'min-h-[112px] px-4 py-3',
        color.border,
        color.panel,
        color.glow,
        tone.ring,
      ].join(' ')}
    >
      {animated && (
        <>
          <div className="pointer-events-none absolute inset-y-0 -left-1/3 w-1/2 -skew-x-12 bg-gradient-to-r from-transparent via-white/[0.08] to-transparent animate-[flow-card-sweep_5.8s_ease-in-out_infinite]" />
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(255,255,255,0.05),transparent_52%)] opacity-60" />
        </>
      )}
      <div className={`absolute inset-x-6 top-0 h-px bg-gradient-to-r ${color.rail}`} />
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-[10px] uppercase tracking-[0.22em] text-slate-500">Stage</div>
          <div className={`mt-1 font-semibold text-slate-100 ${hero ? 'text-base' : 'text-sm'}`}>{title}</div>
        </div>
        <div className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-1 text-[10px] uppercase tracking-[0.16em] ${tone.badge}`}>
          <span className={`h-1.5 w-1.5 rounded-full ${tone.dot}`} />
          {tone.label}
        </div>
      </div>

      <div className="mt-5 flex items-end justify-between gap-3">
        <div>
          <div className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Latency</div>
          <div className={`mt-1 font-semibold ${hero ? 'text-2xl' : 'text-xl'} ${color.accent}`}>
            {typeof latency === 'number' ? `${latency}ms` : '--'}
          </div>
        </div>
        <div className="text-right">
          <div className="text-[10px] uppercase tracking-[0.16em] text-slate-500">Role</div>
          <div className="mt-1 text-xs text-slate-300">{subtitle}</div>
        </div>
      </div>

      {(statusLine || detailLine) && (
        <div className="mt-4 border-t border-white/5 pt-3">
          {statusLine && <div className="text-[11px] font-medium tracking-[0.08em] text-slate-300">{statusLine}</div>}
          {detailLine && <div className="mt-1 text-[11px] text-slate-500">{detailLine}</div>}
        </div>
      )}
    </div>
  );
}

function SourcePill({
  label,
  latency,
  accent,
  ok,
}: {
  label: string;
  latency?: number;
  accent: string;
  ok?: boolean;
}) {
  const tone = getStatusTone(ok, latency);

  return (
    <div className={`rounded-2xl border px-3 py-2.5 ${STAGE_COLORS.sources.pill} ${tone.ring}`}>
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <span className={`h-2 w-2 rounded-full ${accent}`} />
          <span className="text-xs font-semibold tracking-[0.14em] text-slate-100">{label}</span>
        </div>
        <span className="text-xs text-slate-400">{typeof latency === 'number' ? `${latency}ms` : '--'}</span>
      </div>
    </div>
  );
}

function SidecarPill({
  label,
  latency,
  ok,
}: {
  label: string;
  latency?: number;
  ok?: boolean;
}) {
  const tone = getStatusTone(ok, latency);

  return (
    <div className={`rounded-2xl border px-3 py-2.5 ${STAGE_COLORS.sidecars.pill} ${tone.ring}`}>
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <span className={`h-1.5 w-1.5 rounded-full ${tone.dot}`} />
          <span className="text-xs font-semibold tracking-[0.14em] text-slate-100">{label}</span>
        </div>
        <span className="text-xs text-slate-400">{typeof latency === 'number' ? `${latency}ms` : '--'}</span>
      </div>
    </div>
  );
}

function BlockConnector({
  from,
  to,
  mobileLabel,
  dotClass,
}: {
  from: string;
  to: string;
  mobileLabel: string;
  dotClass: string;
}) {
  return (
    <>
      <div className="xl:hidden flex items-center justify-center py-1">
        <div className="inline-flex items-center gap-2 rounded-full border border-slate-700/70 bg-slate-900/60 px-3 py-1 text-[10px] uppercase tracking-[0.18em] text-slate-400">
          <span className={`h-1.5 w-1.5 rounded-full ${dotClass}`} />
          {mobileLabel}
        </div>
      </div>
      <div className="hidden xl:flex items-center justify-center">
        <div className="relative h-[2px] w-16 rounded-full bg-slate-800/90">
          <div className={`absolute inset-y-0 left-0 w-full rounded-full bg-gradient-to-r ${from} ${to} opacity-70`} />
          <div className={`absolute top-1/2 h-2 w-2 -translate-y-1/2 rounded-full ${dotClass} shadow-[0_0_14px_rgba(255,255,255,0.35)] animate-[flow-connector_2.8s_ease-in-out_infinite]`} />
        </div>
      </div>
    </>
  );
}

export default function ServiceFlowDiagram({
  services = [],
  executionMode = 'paper',
  executorState = 'down',
  flowContext,
  activeSignalsCount = 0,
  openPositionsCount = 0,
  closedTradesCount = 0,
}: ServiceFlowDiagramProps) {
  const serviceState = useMemo(() => {
    const bybit = getService(services, 'bybit');
    const binance = getService(services, 'binance');
    const okx = getService(services, 'okx');
    const marketData = getService(services, 'market-data');
    const strategy = getService(services, 'strategy');
    const executor = getService(services, 'executor');
    const api = getService(services, 'api');
    const monitor = getService(services, 'monitor');
    const analytics = getService(services, 'analytics');
    const backtest = getService(services, 'backtest');

    return {
      bybit,
      binance,
      okx,
      marketData,
      strategy,
      executor: {
        ok: executorState === 'running' ? true : executorState === 'paused' ? true : executor?.ok ?? false,
        latency_ms: executor?.latency_ms ?? 0,
      },
      api,
      monitor,
      analytics,
      backtest,
    };
  }, [executorState, services]);

  const marketTone = getStatusTone(serviceState.marketData?.ok, serviceState.marketData?.latency_ms);
  const strategyTone = getStatusTone(serviceState.strategy?.ok, serviceState.strategy?.latency_ms);
  const executorTone = getStatusTone(serviceState.executor?.ok, serviceState.executor?.latency_ms);
  const showMultiSource = executionMode !== 'testnet';
  const marketStatusLine = `${activeSignalsCount} active ${activeSignalsCount === 1 ? 'symbol' : 'symbols'} · ${showMultiSource ? 'multi venue' : 'single venue'}`;
  const strategyStatusLine = flowContext?.strategySymbol
    ? `${flowContext.strategySymbol} · ${flowContext.strategyState || 'Scanning'}`
    : 'Scanning market conditions';
  const strategyDetailLine = flowContext?.strategyContext || `${closedTradesCount} closed trades observed`;
  const executorStatusLine = flowContext?.executorSymbol
    ? `${flowContext.executorSymbol} · ${flowContext.executorAction || 'idle'}`
    : `${openPositionsCount} open ${openPositionsCount === 1 ? 'position' : 'positions'}`;
  const executorDetailLine = flowContext?.executorContext || 'Awaiting valid execution pressure';

  return (
    <div className="grid gap-4 xl:grid-cols-[0.9fr_auto_1.9fr_auto_0.9fr] xl:items-stretch">
      <style jsx>{`
        @keyframes flow-connector {
          0% {
            left: 0%;
            opacity: 0;
            transform: translate(0, -50%) scale(0.9);
          }
          12% {
            opacity: 1;
          }
          88% {
            opacity: 1;
          }
          100% {
            left: calc(100% - 0.5rem);
            opacity: 0;
            transform: translate(0, -50%) scale(1.05);
          }
        }

        @keyframes flow-card-sweep {
          0% {
            transform: translateX(-150%) skewX(-12deg);
            opacity: 0;
          }
          18% {
            opacity: 1;
          }
          100% {
            transform: translateX(320%) skewX(-12deg);
            opacity: 0;
          }
        }
      `}</style>
      <div className="rounded-[28px] border border-slate-700/70 bg-slate-950/45 p-4 shadow-[0_20px_50px_rgba(2,6,23,0.32)]">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Sources</div>
            <div className="mt-1 text-sm font-semibold text-slate-100">
              {showMultiSource ? 'Exchange Feeds' : 'Execution Venue'}
            </div>
          </div>
          <div className="rounded-full border border-slate-700/70 px-2 py-1 text-[10px] uppercase tracking-[0.16em] text-slate-400">
            {showMultiSource ? '3 venues' : 'single venue'}
          </div>
        </div>

        <div className="space-y-3">
          {showMultiSource && (
            <SourcePill label="BINANCE" latency={serviceState.binance?.latency_ms} accent="bg-slate-100" ok={serviceState.binance?.ok} />
          )}
          <SourcePill label="BYBIT" latency={serviceState.bybit?.latency_ms} accent="bg-amber-400" ok={serviceState.bybit?.ok} />
          {showMultiSource && (
            <SourcePill label="OKX" latency={serviceState.okx?.latency_ms} accent="bg-slate-300" ok={serviceState.okx?.ok} />
          )}
        </div>
      </div>

      <BlockConnector
        from="from-slate-300/15 via-sky-400/55"
        to="to-violet-400/35"
        mobileLabel="market flow"
        dotClass={strategyTone.dot}
      />

      <div className="relative overflow-hidden rounded-[28px] border border-slate-700/70 bg-slate-950/45 p-4 shadow-[0_24px_56px_rgba(2,6,23,0.32)]">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Core pipeline</div>
            <div className="mt-1 text-sm font-semibold text-slate-100">Market Data to Execution</div>
          </div>
          <div className="rounded-full border border-slate-700/70 px-2 py-1 text-[10px] uppercase tracking-[0.16em] text-slate-400">
            runtime path
          </div>
        </div>

        <div className="pointer-events-none absolute left-8 right-8 top-[74px] hidden h-px bg-gradient-to-r from-sky-400/20 via-violet-400/20 to-emerald-400/20 lg:block" />

        <div className="grid gap-3 lg:grid-cols-3">
          <StageCard
            title="Market"
            latency={serviceState.marketData?.latency_ms}
            subtitle="normalize + publish"
            statusLine={marketStatusLine}
            detailLine={serviceState.marketData?.ok === false ? 'Feed normalization degraded' : 'Venue telemetry is flowing into the runtime path'}
            color={STAGE_COLORS.marketData}
            tone={marketTone}
            animated
          />
          <StageCard
            title="Strategy"
            latency={serviceState.strategy?.latency_ms}
            subtitle="score + decide"
            statusLine={strategyStatusLine}
            detailLine={strategyDetailLine}
            color={STAGE_COLORS.strategy}
            tone={strategyTone}
            animated
          />
          <StageCard
            title="Executor"
            latency={serviceState.executor?.latency_ms}
            subtitle={executorState === 'paused' ? 'orders paused' : 'orders + reconcile'}
            statusLine={executorStatusLine}
            detailLine={executorDetailLine}
            color={STAGE_COLORS.executor}
            tone={executorTone}
            hero
            animated
          />
        </div>
      </div>

      <BlockConnector
        from="from-violet-400/35 via-emerald-400/55"
        to="to-blue-400/25"
        mobileLabel="execution fan-out"
        dotClass={executorTone.dot}
      />

      <div className="rounded-[28px] border border-slate-700/70 bg-slate-950/45 p-4 shadow-[0_20px_50px_rgba(2,6,23,0.28)]">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <div className="text-[10px] uppercase tracking-[0.2em] text-slate-500">Sidecars</div>
            <div className="mt-1 text-sm font-semibold text-slate-100">State and Analysis</div>
          </div>
          <div className="rounded-full border border-slate-700/70 px-2 py-1 text-[10px] uppercase tracking-[0.16em] text-slate-400">
            observers
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-1">
          <SidecarPill label="API" latency={serviceState.api?.latency_ms} ok={serviceState.api?.ok} />
          <SidecarPill label="MONITOR" latency={serviceState.monitor?.latency_ms} ok={serviceState.monitor?.ok} />
          <SidecarPill label="ANALYTICS" latency={serviceState.analytics?.latency_ms} ok={serviceState.analytics?.ok} />
          <SidecarPill label="BACKTEST" latency={serviceState.backtest?.latency_ms} ok={serviceState.backtest?.ok} />
        </div>
      </div>
    </div>
  );
}
