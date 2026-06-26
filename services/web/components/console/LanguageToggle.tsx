'use client';

import { cn } from '@/lib/utils';
import { LOCALES, LOCALE_LABEL, setLocale, useLocale, useT } from '@/lib/i18n';

/**
 * pt-BR ↔ en language switch. Same preference pattern as DensityToggle: the DOM
 * (`data-locale` on <html>) is the source of truth — set pre-hydration by the root
 * layout's inline script and read via useSyncExternalStore (server snapshot = pt-BR),
 * so there's no effect-driven setState and no hydration flash. No URL/route changes.
 */
export function LanguageToggle({ className }: { className?: string }) {
  const locale = useLocale();
  const t = useT('common');

  return (
    <div
      className={cn('flex items-center rounded-md border border-border/60 p-0.5', className)}
      role="group"
      aria-label={t('language')}
    >
      {LOCALES.map(l => (
        <button
          key={l}
          type="button"
          onClick={() => setLocale(l)}
          aria-pressed={locale === l}
          className={cn(
            'rounded px-1.5 py-0.5 text-[11px] font-medium tabular-nums transition-colors',
            locale === l
              ? 'bg-secondary text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          )}
        >
          {LOCALE_LABEL[l]}
        </button>
      ))}
    </div>
  );
}
