'use client';

import { useId } from 'react';
import { cn } from '@/lib/utils';

interface SparklineProps {
  /** Y-values in draw order (left → right). Needs ≥2 points to render a line. */
  values: number[];
  width?: number;
  height?: number;
  className?: string;
  /** Tailwind text-color class driving stroke + fill (via currentColor). */
  colorClassName?: string;
  /** Draw a faint baseline at y=0 when the series crosses zero. */
  showZero?: boolean;
}

/**
 * Minimal dependency-free SVG sparkline. Pure presentation — the caller owns
 * what the series means (here: a cumulative realized-PnL curve).
 */
export function Sparkline({
  values,
  width = 132,
  height = 40,
  className,
  colorClassName = 'text-accent',
  showZero = true,
}: SparklineProps) {
  const gradientId = useId();

  if (values.length < 2) {
    return (
      <div
        className={cn('flex items-center justify-center', className)}
        style={{ width, height }}
      >
        <span className="text-[10px] text-muted-foreground">no data yet</span>
      </div>
    );
  }

  const pad = 2;
  const min = Math.min(...values, showZero ? 0 : Infinity);
  const max = Math.max(...values, showZero ? 0 : -Infinity);
  const span = max - min || 1;

  const x = (i: number) =>
    pad + (i / (values.length - 1)) * (width - pad * 2);
  const y = (v: number) =>
    height - pad - ((v - min) / span) * (height - pad * 2);

  const line = values.map((v, i) => `${x(i)},${y(v)}`).join(' ');
  const area = `${pad},${height - pad} ${line} ${width - pad},${height - pad}`;
  const zeroY = y(0);

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={cn(colorClassName, className)}
      role="img"
      aria-hidden
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.22" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
      </defs>
      {showZero && zeroY > pad && zeroY < height - pad && (
        <line
          x1={pad}
          y1={zeroY}
          x2={width - pad}
          y2={zeroY}
          stroke="currentColor"
          strokeOpacity="0.18"
          strokeDasharray="2 3"
        />
      )}
      <polygon points={area} fill={`url(#${gradientId})`} />
      <polyline
        points={line}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}
