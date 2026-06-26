'use client';

import { useMemo } from 'react';
import {
  Area,
  AreaChart,
  CartesianGrid,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { useDashboard } from '@/hooks/useDashboard';
import { useLocale, useT, formatUsd, type Locale } from '@/lib/i18n';
import { SectionCard } from '@/components/ui/SectionCard';
import type { Trade } from '@/types/trading';

type T = ReturnType<typeof useT<'console'>>;

interface Point {
  i: number;
  cum: number;
  pnl: number;
  symbol: string;
  side: string;
  reason: string;
  when: string;
}

function titleCase(value?: string | null) {
  if (!value) return 'closed';
  return value
    .replaceAll('_', ' ')
    .toLowerCase()
    .replace(/\b\w/g, c => c.toUpperCase());
}

function CurveTooltip({ active, payload, t, locale }: any) {
  if (!active || !payload?.length) return null;
  const p: Point = payload[0].payload;
  const win = p.pnl >= 0;
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2 text-xs shadow-lg">
      <div className="font-semibold text-foreground">
        {p.symbol}{' '}
        <span className={win ? 'text-accent' : 'text-destructive'}>
          {p.side}
        </span>
      </div>
      <div className="mt-1 font-mono tabular-nums text-muted-foreground">
        {(t as T)('fill')}{' '}
        <span className={win ? 'text-accent' : 'text-destructive'}>
          {formatUsd(locale as Locale, p.pnl)}
        </span>{' '}
        · {titleCase(p.reason)}
      </div>
      <div className="mt-0.5 font-mono tabular-nums text-foreground/80">
        {(t as T)('equityLabel')} {formatUsd(locale as Locale, p.cum)}
      </div>
      <div className="mt-0.5 text-[11px] text-muted-foreground">{p.when}</div>
    </div>
  );
}

/** Colored marker per fill — green above the line for a win, red for a loss. */
function FillDot(props: any) {
  const { cx, cy, payload } = props;
  if (typeof cx !== 'number' || typeof cy !== 'number') return null;
  const win = (payload as Point).pnl >= 0;
  return (
    <circle
      cx={cx}
      cy={cy}
      r={2.5}
      fill={win ? '#00ff88' : '#ef4444'}
      stroke="#0a1120"
      strokeWidth={0.5}
    />
  );
}

/**
 * Realized-PnL equity curve with a marker per fill — the chart operators ask
 * for first (§6.4). Cumulative realized PnL of closed trades (oldest → newest);
 * hover a marker for the fill + its close-reason. This is realized PnL, not
 * mark-to-market equity (no equity time-series exists in the API).
 */
export function EquityCurve() {
  const t = useT('console');
  const locale = useLocale();
  const { data, loading, error } = useDashboard<{ items: Trade[] }>(
    '/api/v1/trades?limit=200',
    { refreshInterval: 15000 }
  );

  const { points, net, peak, low } = useMemo(() => {
    const closed = (data?.items ?? [])
      .filter(t => t.status === 'closed')
      .sort(
        (a, b) =>
          Date.parse(a.closed_at || a.opened_at) -
          Date.parse(b.closed_at || b.opened_at)
      );
    const pts: Point[] = [];
    let cum = 0;
    let hi = 0;
    let lo = 0;
    for (let i = 0; i < closed.length; i++) {
      const t = closed[i];
      cum += t.pnl ?? 0;
      hi = Math.max(hi, cum);
      lo = Math.min(lo, cum);
      const d = new Date(t.closed_at || t.opened_at);
      pts.push({
        i,
        cum: Number(cum.toFixed(4)),
        pnl: t.pnl ?? 0,
        symbol: t.symbol,
        side: t.side,
        reason: t.close_reason || 'closed',
        when: Number.isNaN(d.getTime())
          ? '—'
          : d.toLocaleString(locale, {
              month: 'short',
              day: 'numeric',
              hour: '2-digit',
              minute: '2-digit',
            }),
      });
    }
    return { points: pts, net: cum, peak: hi, low: lo };
  }, [data, locale]);

  // Fraction of the chart height where $0 sits, so the fill/stroke can switch
  // green (above) → red (below) exactly at the zero line — a net-negative curve
  // reads red instead of pooling cyan near the top.
  const top = Math.max(peak, 0);
  const bottom = Math.min(low, 0);
  const zeroOffset = top - bottom > 0 ? top / (top - bottom) : 1;

  return (
    <SectionCard>
      <div className="mb-3 flex flex-wrap items-end justify-between gap-2">
        <div>
          <h3 className="text-base font-semibold text-foreground">
            {t('equityTitle')}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t('equitySubtitle', { n: points.length })}
          </p>
        </div>
        <div className="flex items-center gap-4 font-mono text-sm tabular-nums">
          <span className="text-muted-foreground">
            {t('equityNet')}{' '}
            <span className={net >= 0 ? 'text-accent' : 'text-destructive'}>
              {formatUsd(locale, net)}
            </span>
          </span>
          <span className="text-muted-foreground">
            {t('equityPeak')}{' '}
            <span className="text-foreground">{formatUsd(locale, peak)}</span>
          </span>
        </div>
      </div>

      {error ? (
        <div className="flex h-64 items-center justify-center rounded-lg bg-secondary/40 text-sm text-destructive">
          {error}
        </div>
      ) : loading && points.length === 0 ? (
        <div className="h-64 animate-pulse rounded-lg bg-secondary/40" />
      ) : points.length === 0 ? (
        <div className="flex h-64 items-center justify-center rounded-lg bg-secondary/40 text-sm text-muted-foreground">
          {t('equityEmpty')}
        </div>
      ) : (
        <div className="h-64">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart
              data={points}
              margin={{ top: 8, right: 8, bottom: 0, left: 0 }}
            >
              <defs>
                {/* Fill: green above the zero line, red below it. */}
                <linearGradient id="equityFill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset={0} stopColor="#10b981" stopOpacity={0.3} />
                  <stop
                    offset={zeroOffset}
                    stopColor="#10b981"
                    stopOpacity={0.04}
                  />
                  <stop
                    offset={zeroOffset}
                    stopColor="#ef4444"
                    stopOpacity={0.04}
                  />
                  <stop offset={1} stopColor="#ef4444" stopOpacity={0.3} />
                </linearGradient>
                {/* Stroke: the line itself turns red while underwater. */}
                <linearGradient id="equityStroke" x1="0" y1="0" x2="0" y2="1">
                  <stop offset={zeroOffset} stopColor="#10b981" />
                  <stop offset={zeroOffset} stopColor="#ef4444" />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
              <XAxis
                dataKey="i"
                stroke="#64748b"
                fontSize={11}
                tickFormatter={() => ''}
                tickLine={false}
              />
              <YAxis
                stroke="#64748b"
                fontSize={11}
                width={48}
                tickFormatter={v => `$${v}`}
              />
              <ReferenceLine y={0} stroke="#475569" strokeDasharray="2 4" />
              <Tooltip content={<CurveTooltip t={t} locale={locale} />} />
              <Area
                type="monotone"
                dataKey="cum"
                baseValue={0}
                stroke="url(#equityStroke)"
                strokeWidth={2}
                fill="url(#equityFill)"
                dot={<FillDot />}
                activeDot={{ r: 4 }}
                isAnimationActive={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      )}
    </SectionCard>
  );
}
