import { cn } from '@/lib/utils';

/**
 * Mission-Control instrument panel — the HUD counterpart of {@link SectionCard}
 * (same role, new chrome). A bordered card with a gradient top rail (from the
 * `.hud-frame` class in globals.css) and four cyan corner brackets, plus an
 * optional uppercase display-font header with a `right` slot.
 *
 * Brackets and rail are purely decorative (`aria-hidden`). Pass `scan` to add
 * the ambient scanline sweep (opt-in; honors prefers-reduced-motion).
 */
function Corner({ className }: { className: string }) {
  return (
    <span
      aria-hidden
      className={cn(
        'pointer-events-none absolute h-3 w-3 border-primary/60',
        className
      )}
    />
  );
}

export function HudFrame({
  title,
  right,
  tone = 'default',
  scan = false,
  className,
  children,
}: {
  title?: string;
  right?: React.ReactNode;
  tone?: 'default' | 'accent' | 'danger';
  scan?: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  const toneBorder =
    tone === 'accent'
      ? 'border-accent/40'
      : tone === 'danger'
        ? 'border-destructive/40'
        : 'border-border';

  return (
    <section
      className={cn(
        'hud-frame rounded-md border bg-card/60 p-4',
        toneBorder,
        scan && 'hud-scan',
        className
      )}
    >
      <Corner className="left-0 top-0 border-l border-t" />
      <Corner className="right-0 top-0 border-r border-t" />
      <Corner className="bottom-0 left-0 border-b border-l" />
      <Corner className="bottom-0 right-0 border-b border-r" />

      {title && (
        <div className="mb-3 flex items-center justify-between gap-3">
          <span className="font-display text-[11px] uppercase tracking-[0.25em] text-primary/80">
            {title}
          </span>
          {right && (
            <span className="font-mono text-[11px] tabular-nums text-muted-foreground">
              {right}
            </span>
          )}
        </div>
      )}
      {children}
    </section>
  );
}
