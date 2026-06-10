'use client';

import { useSyncExternalStore } from 'react';
import { cn } from '@/lib/utils';

type Density = 'comfortable' | 'cockpit';

const CHANGE_EVENT = 'viper:density-change';

function read(): Density {
  return document.documentElement.dataset.density === 'cockpit'
    ? 'cockpit'
    : 'comfortable';
}

function subscribe(cb: () => void) {
  window.addEventListener(CHANGE_EVENT, cb);
  window.addEventListener('storage', cb);
  return () => {
    window.removeEventListener(CHANGE_EVENT, cb);
    window.removeEventListener('storage', cb);
  };
}

/**
 * Comfortable ↔ Cockpit density (§4.4). The DOM (`data-density` on <html>) is the
 * source of truth — set pre-hydration by the root layout's inline script and
 * read here via useSyncExternalStore (server snapshot = comfortable), so there's
 * no effect-driven setState and no hydration flash. Toggling rewrites the
 * attribute, persists it, and notifies subscribers.
 */
export function DensityToggle({ className }: { className?: string }) {
  const density = useSyncExternalStore(
    subscribe,
    read,
    () => 'comfortable' as Density
  );

  function toggle() {
    const next: Density = density === 'comfortable' ? 'cockpit' : 'comfortable';
    if (next === 'cockpit') {
      document.documentElement.dataset.density = 'cockpit';
    } else {
      delete document.documentElement.dataset.density;
    }
    try {
      localStorage.setItem('viper-density', next);
    } catch {
      /* storage unavailable */
    }
    window.dispatchEvent(new Event(CHANGE_EVENT));
  }

  return (
    <button
      type="button"
      onClick={toggle}
      title={`Density: ${density} — click for ${density === 'comfortable' ? 'cockpit' : 'comfortable'}`}
      aria-label="Toggle density"
      className={cn(
        'hidden items-center rounded-md border border-border px-2 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground sm:inline-flex',
        className
      )}
    >
      {density === 'cockpit' ? 'Cockpit' : 'Comfort'}
    </button>
  );
}
