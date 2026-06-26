// Supported UI locales. Default is pt-BR (the operator's language); en is the
// type source-of-truth for the message catalogs (see messages/).
export const LOCALES = ['pt-BR', 'en'] as const;
export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = 'pt-BR';

export const LOCALE_LABEL: Record<Locale, string> = {
  'pt-BR': 'PT',
  en: 'EN',
};

export function isLocale(value: string | null | undefined): value is Locale {
  return value === 'pt-BR' || value === 'en';
}

// localStorage key + DOM event, mirroring the density preference pattern
// (components/console/DensityToggle.tsx) so language is a no-flash, persisted,
// DOM-attribute-driven preference with no routing changes.
export const LOCALE_STORAGE_KEY = 'viper-locale';
export const LOCALE_CHANGE_EVENT = 'viper:locale-change';
