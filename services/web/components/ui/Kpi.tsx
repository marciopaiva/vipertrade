import { cn } from '@/lib/utils';

/**
 * One labelled metric box. Extracted from the identical `rounded-lg border-border
 * bg-card p-3` stat used across LiveQualityTab / TuningTab / KpiStrip so every
 * screen renders KPIs the same way. `tone` colors the value (e.g. text-accent /
 * text-destructive for signed PnL).
 */
export function Kpi({
  label,
  value,
  tone,
  className,
}: {
  label: string;
  value: string;
  tone?: string;
  className?: string;
}) {
  return (
    <div className={cn('rounded-lg border border-border bg-card p-3', className)}>
      <div className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">{label}</div>
      <div className={cn('mt-1 font-mono text-lg tabular-nums', tone ?? 'text-foreground')}>
        {value}
      </div>
    </div>
  );
}
