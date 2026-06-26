'use client';

import { useSyncExternalStore } from 'react';
import {
  DEFAULT_LOCALE,
  Locale,
  LOCALE_CHANGE_EVENT,
  LOCALE_STORAGE_KEY,
  isLocale,
} from './locales';
import { messages, Messages } from './messages';

export type { Locale } from './locales';
export { LOCALES, LOCALE_LABEL, DEFAULT_LOCALE } from './locales';
export * from './format';

// Active locale lives on <html data-locale> (DOM = source of truth, set pre-hydration
// by the inline script in app/layout.tsx), read via useSyncExternalStore — identical to
// the density preference pattern (components/console/DensityToggle.tsx), so there's no
// React context, no effect-driven setState, and no hydration flash.
function read(): Locale {
  if (typeof document === 'undefined') return DEFAULT_LOCALE;
  const value = document.documentElement.dataset.locale;
  return isLocale(value) ? value : DEFAULT_LOCALE;
}

function subscribe(callback: () => void): () => void {
  window.addEventListener(LOCALE_CHANGE_EVENT, callback);
  window.addEventListener('storage', callback);
  return () => {
    window.removeEventListener(LOCALE_CHANGE_EVENT, callback);
    window.removeEventListener('storage', callback);
  };
}

export function useLocale(): Locale {
  return useSyncExternalStore(subscribe, read, () => DEFAULT_LOCALE);
}

/** Set the active locale: rewrite the DOM attr + lang, persist, and notify subscribers. */
export function setLocale(next: Locale): void {
  document.documentElement.dataset.locale = next;
  document.documentElement.lang = next;
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, next);
  } catch {
    /* storage unavailable */
  }
  window.dispatchEvent(new Event(LOCALE_CHANGE_EVENT));
}

type Vars = Record<string, string | number>;

/**
 * Translator for a namespace: `const t = useT('analysis'); t('title')`. Supports
 * `{var}` interpolation. Re-renders when the locale changes.
 */
export function useT<N extends keyof Messages>(
  namespace: N
): (key: keyof Messages[N], vars?: Vars) => string {
  const locale = useLocale();
  const dict = messages[locale][namespace];
  return (key, vars) => {
    let out = String(dict[key]);
    if (vars) {
      for (const [k, v] of Object.entries(vars)) {
        out = out.split(`{${k}}`).join(String(v));
      }
    }
    return out;
  };
}
