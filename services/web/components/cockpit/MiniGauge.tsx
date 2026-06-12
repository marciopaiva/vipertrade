'use client';

import { cn } from '@/lib/utils';
import type { GaugeZone } from './GaugeBar';

const pct = (v: number, min: number, max: number) =>
  Math.max(0, Math.min(100, ((v - min) / (max - min)) * 100));

/**
 * Compact one-line indicator: the value, an optional ⚠ flag, and a short track
 * with shaded zones + a marker. The row-layout counterpart of GaugeBar — the
 * column header carries the label, so this renders no label of its own.
 */
export function MiniGauge({
  value,
  min,
  max,
  zones = [],
  format,
  tone,
}: {
  value: number | null | undefined;
  min: number;
  max: number;
  zones?: GaugeZone[];
  format?: (v: number) => string;
  /** 'danger' = in a guard zone (red), 'warn' = weak/soft (amber) */
  tone?: 'danger' | 'warn';
}) {
  const has = typeof value === 'number' && Number.isFinite(value);
  const v = has ? (value as number) : min;
  const left = pct(v, min, max);
  const toneText =
    tone === 'danger'
      ? 'text-destructive'
      : tone === 'warn'
        ? 'text-warn'
        : 'text-foreground';

  return (
    <div className="flex items-center gap-1.5">
      <span
        className={cn(
          'w-9 shrink-0 text-right font-mono text-xs tabular-nums',
          toneText
        )}
      >
        {has ? (format ? format(v) : v.toFixed(2)) : '—'}
      </span>
      <span className={cn('w-2 shrink-0 text-[10px] leading-none', toneText)}>
        {tone ? '⚠' : ''}
      </span>
      <div className="relative h-1.5 w-16 shrink-0 overflow-hidden rounded-full bg-secondary">
        {zones.map((z, i) => (
          <div
            key={i}
            className={cn('absolute inset-y-0', z.className)}
            style={{
              left: `${pct(z.from, min, max)}%`,
              width: `${pct(z.to, min, max) - pct(z.from, min, max)}%`,
            }}
          />
        ))}
        {has && (
          <div
            className={cn(
              'absolute top-1/2 h-2.5 w-1 -translate-x-1/2 -translate-y-1/2 rounded-full ring-1 ring-background',
              tone === 'danger' ? 'bg-destructive' : 'bg-primary'
            )}
            style={{ left: `${left}%` }}
          />
        )}
      </div>
    </div>
  );
}
