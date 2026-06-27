import { cn } from '@/lib/utils';

/**
 * Horizontal KPI rail — the HUD readout strip (`▸ LABEL value`), mono with
 * `tabular-nums` so figures don't jitter on live refresh. Evolves the inline
 * stats from `components/console/KpiStrip.tsx` into a reusable instrument rail.
 */
export interface StatRailItem {
  label: string;
  value: React.ReactNode;
  tone?: 'default' | 'accent' | 'danger' | 'warn';
}

const TONE: Record<NonNullable<StatRailItem['tone']>, string> = {
  default: 'text-foreground',
  accent: 'text-accent',
  danger: 'text-destructive',
  warn: 'text-warn',
};

export function StatRail({
  items,
  className,
}: {
  items: StatRailItem[];
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-wrap items-center gap-x-6 gap-y-2 font-mono text-sm tabular-nums',
        className
      )}
    >
      {items.map((it, i) => (
        <span key={i} className="inline-flex items-center gap-1.5">
          <span aria-hidden className="text-primary/70">
            ▸
          </span>
          <span className="font-display text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            {it.label}
          </span>
          <span className={cn('font-semibold', TONE[it.tone ?? 'default'])}>
            {it.value}
          </span>
        </span>
      ))}
    </div>
  );
}
