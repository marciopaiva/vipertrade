'use client';

import { cn } from '@/lib/utils';

export interface GaugeZone {
  from: number;
  to: number;
  /** tailwind bg color class for the zone band */
  className: string;
}

interface GaugeBarProps {
  label: string;
  value: number | null | undefined;
  min: number;
  max: number;
  zones?: GaugeZone[];
  /** optional unit/format for the printed value */
  format?: (v: number) => string;
  /** highlight the marker when the value sits inside a danger zone */
  danger?: boolean;
}

const pct = (v: number, min: number, max: number) =>
  Math.max(0, Math.min(100, ((v - min) / (max - min)) * 100));

/**
 * Horizontal indicator track with shaded guard zones and a value marker.
 * Used for RSI (overbought/oversold) and Bollinger %B (entry-guard zones).
 */
export function GaugeBar({
  label,
  value,
  min,
  max,
  zones = [],
  format,
  danger,
}: GaugeBarProps) {
  const has = typeof value === 'number' && Number.isFinite(value);
  const v = has ? (value as number) : min;
  const left = pct(v, min, max);

  return (
    <div className="space-y-1">
      <div className="flex items-baseline justify-between text-xs">
        <span className="font-medium uppercase tracking-wide text-muted-foreground">
          {label}
        </span>
        <span
          className={cn(
            'font-mono tabular-nums',
            danger ? 'text-destructive' : 'text-foreground'
          )}
        >
          {has ? (format ? format(v) : v.toFixed(2)) : '—'}
        </span>
      </div>
      <div className="relative h-2 w-full overflow-hidden rounded-full bg-secondary">
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
              'absolute top-1/2 h-3.5 w-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-background',
              danger ? 'bg-destructive' : 'bg-primary'
            )}
            style={{ left: `${left}%` }}
          />
        )}
      </div>
    </div>
  );
}
