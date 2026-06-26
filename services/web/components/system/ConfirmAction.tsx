'use client';

import { useState } from 'react';
import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';

/**
 * Two-step guard for consequential actions (kill-switch, executor). First click
 * arms a confirm/cancel pair; only the second commits. While the action runs the
 * button shows a pending state. Disabled actions explain why on hover.
 */
export function ConfirmAction({
  label,
  confirmLabel,
  onConfirm,
  tone = 'default',
  disabled = false,
  disabledReason,
  className,
}: {
  label: string;
  confirmLabel?: string;
  onConfirm: () => Promise<void> | void;
  tone?: 'default' | 'danger';
  disabled?: boolean;
  disabledReason?: string;
  className?: string;
}) {
  const tc = useT('common');
  const [armed, setArmed] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function commit() {
    setPending(true);
    setError(null);
    try {
      await onConfirm();
      setArmed(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPending(false);
    }
  }

  if (disabled) {
    return (
      <span
        title={disabledReason}
        className={cn(
          'inline-flex cursor-not-allowed items-center rounded-md border border-border bg-secondary/40 px-3 py-1.5 text-xs font-medium text-muted-foreground opacity-70',
          className
        )}
      >
        {label}
      </span>
    );
  }

  if (!armed) {
    return (
      <button
        type="button"
        onClick={() => setArmed(true)}
        className={cn(
          'inline-flex items-center rounded-md border px-3 py-1.5 text-xs font-medium transition-colors',
          tone === 'danger'
            ? 'border-destructive/40 bg-destructive/10 text-destructive hover:bg-destructive/15'
            : 'border-border bg-card text-foreground hover:border-primary/40',
          className
        )}
      >
        {label}
      </button>
    );
  }

  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <span className="text-xs text-muted-foreground">{tc('sure')}</span>
      <button
        type="button"
        disabled={pending}
        onClick={commit}
        className={cn(
          'inline-flex items-center rounded-md px-3 py-1.5 text-xs font-semibold text-white transition-colors disabled:opacity-60',
          tone === 'danger'
            ? 'bg-destructive hover:bg-destructive/90'
            : 'bg-primary hover:bg-primary/90'
        )}
      >
        {pending ? '…' : (confirmLabel ?? tc('confirm'))}
      </button>
      <button
        type="button"
        disabled={pending}
        onClick={() => {
          setArmed(false);
          setError(null);
        }}
        className="inline-flex items-center rounded-md border border-border px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground disabled:opacity-60"
      >
        {tc('cancel')}
      </button>
      {error && <span className="text-xs text-destructive">{error}</span>}
    </span>
  );
}
