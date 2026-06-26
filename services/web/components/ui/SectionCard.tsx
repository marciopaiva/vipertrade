import { cn } from '@/lib/utils';

/**
 * A console section card. Extracted from the repeated `rounded-xl border-border
 * bg-card p-5` panel + its `text-[10px] uppercase tracking-[0.2em]` header used all
 * over the analysis/console screens, so sections share one rhythm. Pass `title`
 * for the standard header (with optional `right` slot), or omit it for a bare card.
 */
export function SectionCard({
  title,
  right,
  children,
  className,
  tone = 'default',
}: {
  title?: string;
  right?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  tone?: 'default' | 'accent';
}) {
  return (
    <section
      className={cn(
        'rounded-xl border bg-card p-5',
        tone === 'accent' ? 'border-accent/30 bg-accent/5' : 'border-border',
        className
      )}
    >
      {title && (
        <div className="mb-3 flex items-center justify-between gap-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
          <span>{title}</span>
          {right && <span className="normal-case tracking-normal">{right}</span>}
        </div>
      )}
      {children}
    </section>
  );
}
