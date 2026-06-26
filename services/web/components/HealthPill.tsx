'use client';

import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';
import { useConnectionStatus, type LiveStatus } from '@/hooks/useConnectionStatus';

const CONFIG: Record<
  LiveStatus,
  { labelKey: 'healthLive' | 'healthConnecting' | 'healthStale' | 'healthOffline'; dot: string; text: string; pulse: boolean }
> = {
  live: {
    labelKey: 'healthLive',
    dot: 'bg-viper-green',
    text: 'text-viper-green',
    pulse: true,
  },
  connecting: {
    labelKey: 'healthConnecting',
    dot: 'bg-viper-cyan',
    text: 'text-viper-cyan',
    pulse: true,
  },
  stale: {
    labelKey: 'healthStale',
    dot: 'bg-viper-orange',
    text: 'text-viper-orange',
    pulse: false,
  },
  down: {
    labelKey: 'healthOffline',
    dot: 'bg-viper-red',
    text: 'text-viper-red',
    pulse: false,
  },
};

/**
 * Global connection-health indicator. Live is the resting state; anything else
 * (stale / offline) announces itself, per the "real-time is the default,
 * failure is loud" principle.
 */
export function HealthPill({ className }: { className?: string }) {
  const t = useT('app');
  const { status } = useConnectionStatus();
  const c = CONFIG[status];
  const label = t(c.labelKey);

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full border border-border/60 px-2.5 py-1 font-mono text-[11px] tracking-wider',
        c.text,
        className
      )}
      title={t('healthFeed', { status: label.toLowerCase() })}
      aria-live="polite"
    >
      <span className="relative flex h-2 w-2">
        {c.pulse && (
          <span
            className={cn(
              'absolute inline-flex h-full w-full animate-ping rounded-full opacity-75',
              c.dot
            )}
          />
        )}
        <span className={cn('relative inline-flex h-2 w-2 rounded-full', c.dot)} />
      </span>
      {label}
    </span>
  );
}
