'use client';

import { useId } from 'react';
import { cn } from '@/lib/utils';

/**
 * Half-dome radial dial — generalized from the Fear & Greed gauge in
 * `components/console/MarketSentiment.tsx` (same `arcPoint` math + dotted scale
 * + marker), so sentiment, peak-capture and win-rate all read as one instrument
 * family. Pass `stops` for a gradient arc (e.g. red→green) or `color` for a
 * solid arc. `sweep` adds a soft pulsing halo on the marker (reduce-motion safe).
 */
export interface GaugeStop {
  offset: number;
  color: string;
}

// Point on the top semicircle for a value in [min,max] (left → right).
function arcPoint(frac: number, radius: number, cx: number, cy: number) {
  const f = Math.min(1, Math.max(0, frac));
  const angle = Math.PI * (1 - f); // π (left) → 0 (right)
  return { x: cx + radius * Math.cos(angle), y: cy - radius * Math.sin(angle) };
}

export function RadialGauge({
  value,
  min = 0,
  max = 100,
  label,
  sublabel,
  color = 'hsl(var(--primary))',
  stops,
  format,
  sweep = false,
  className,
  ariaLabel,
}: {
  value: number | null | undefined;
  min?: number;
  max?: number;
  label?: string;
  sublabel?: string;
  color?: string;
  stops?: GaugeStop[];
  format?: (v: number) => string;
  sweep?: boolean;
  className?: string;
  ariaLabel?: string;
}) {
  const gid = useId().replace(/:/g, '');
  const has = typeof value === 'number' && Number.isFinite(value);
  const v = has ? (value as number) : min;
  const frac = max > min ? (v - min) / (max - min) : 0;

  const cx = 100;
  const cy = 100;
  const R = 80;
  const marker = arcPoint(frac, R, cx, cy);
  const ticks = Array.from({ length: 11 }, (_, i) => arcPoint(i / 10, R - 16, cx, cy));
  const arcColor = stops ? `url(#${gid})` : color;

  const display = has ? (format ? format(v) : String(Math.round(v))) : '—';

  return (
    <div className={cn('relative', className)}>
      <svg
        viewBox="0 0 200 118"
        className="w-full"
        role="img"
        aria-label={ariaLabel ?? `${label ?? ''} ${display}`.trim()}
      >
        {stops && (
          <defs>
            <linearGradient id={gid} x1="0" y1="0" x2="1" y2="0">
              {stops.map((s, i) => (
                <stop key={i} offset={`${s.offset}%`} stopColor={s.color} />
              ))}
            </linearGradient>
          </defs>
        )}

        {/* arc */}
        <path
          d={`M ${cx - R} ${cy} A ${R} ${R} 0 0 1 ${cx + R} ${cy}`}
          fill="none"
          stroke={arcColor}
          strokeWidth={10}
          strokeLinecap="round"
        />

        {/* dotted scale */}
        {ticks.map((t, i) => (
          <circle key={i} cx={t.x} cy={t.y} r={1.6} fill="#475569" />
        ))}

        {/* needle + marker (+ optional pulsing halo) */}
        {has && (
          <>
            <line
              x1={cx}
              y1={cy}
              x2={marker.x}
              y2={marker.y}
              stroke={color}
              strokeWidth={2}
              strokeLinecap="round"
            />
            {sweep && (
              <circle
                cx={marker.x}
                cy={marker.y}
                r={9}
                fill={color}
                className="viper-pulse"
                style={{ transformOrigin: 'center', transformBox: 'fill-box' }}
                opacity={0.25}
              />
            )}
            <circle cx={marker.x} cy={marker.y} r={5} fill={color} />
            <circle cx={cx} cy={cy} r={4} fill={color} />
          </>
        )}
      </svg>

      {/* value + label centered under the dome */}
      <div className="-mt-10 flex flex-col items-center">
        <span
          className="font-mono text-3xl font-bold leading-none tabular-nums"
          style={{ color }}
        >
          {display}
        </span>
        {sublabel && (
          <span className="mt-1 text-sm font-medium" style={{ color }}>
            {sublabel}
          </span>
        )}
        {label && (
          <span className="mt-1 font-display text-[10px] uppercase tracking-[0.22em] text-muted-foreground">
            {label}
          </span>
        )}
      </div>
    </div>
  );
}
