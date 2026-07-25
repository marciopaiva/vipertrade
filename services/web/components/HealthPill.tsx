'use client';

import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';
import {
  useConnectionStatus,
  type LiveStatus,
} from '@/hooks/useConnectionStatus';

const CONFIG: Record<
  LiveStatus,
  {
    labelKey:
      | 'healthLive'
      | 'healthConnecting'
      | 'healthStale'
      | 'healthOffline';
    dot: string;
    text: string;
    pulse: boolean;
  }
> = {
  live: {
    labelKey: 'healthLive',
    dot: 'bg-accent',
    text: 'text-accent',
    pulse: true,
  },
  connecting: {
    labelKey: 'healthConnecting',
    dot: 'bg-primary',
    text: 'text-primary',
    pulse: true,
  },
  stale: {
    labelKey: 'healthStale',
    dot: 'bg-warn',
    text: 'text-warn',
    pulse: false,
  },
  down: {
    labelKey: 'healthOffline',
    dot: 'bg-destructive',
    text: 'text-destructive',
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
        // min-w reserva o espaço do maior rótulo ("CONECTANDO"). Sem isso a
        // pílula encolhe e cresce a cada troca de estado, empurrando o menu e o
        // resto do cabeçalho — o texto muda de largura, o ponto fica ancorado.
        'inline-flex min-w-[6.75rem] items-center gap-1.5 rounded-full border border-border/60 px-2.5 py-1 font-mono text-2xs tracking-wider',
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
        <span
          className={cn('relative inline-flex h-2 w-2 rounded-full', c.dot)}
        />
      </span>
      {label}
    </span>
  );
}
