import { HudFrame } from './HudFrame';

/**
 * A console section panel. Now a thin wrapper over {@link HudFrame} so every
 * screen that used SectionCard inherits the HUD / Mission Control chrome (corner
 * brackets, gradient top rail, display-font header) in one place. Kept as a
 * named export so existing call sites (analysis tabs, EquityCurve) don't change.
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
    <HudFrame title={title} right={right} tone={tone} className={className}>
      {children}
    </HudFrame>
  );
}
