import { Locale } from './locales';

// Locale-aware number formatting (pt-BR comma vs en dot, grouped). These replace
// the ad-hoc `signed`/`fmtPct`/`fmtUsd`/`num` helpers scattered across the
// analysis/console components so every screen formats numbers the same way.
//
// The signed variants keep the app's existing convention: a real minus sign
// (U+2212) and an explicit leading '+' for non-negative values.

export function formatNumber(locale: Locale, value: number, digits = 2): string {
  if (!Number.isFinite(value)) return '—';
  return new Intl.NumberFormat(locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

export function formatSigned(locale: Locale, value: number, digits = 2): string {
  if (!Number.isFinite(value)) return '—';
  const sign = value >= 0 ? '+' : '−';
  return `${sign}${formatNumber(locale, Math.abs(value), digits)}`;
}

/** `value` is already a percentage number (e.g. -0.74 → "−0,74%"). */
export function formatPct(locale: Locale, value: number, digits = 2): string {
  if (!Number.isFinite(value)) return '—';
  return `${formatSigned(locale, value, digits)}%`;
}

/** `value` is a fraction in [0,1] (e.g. 0.52 → "52,0%"). */
export function formatRate(locale: Locale, value: number, digits = 1): string {
  if (!Number.isFinite(value)) return '—';
  return `${formatNumber(locale, value * 100, digits)}%`;
}

/** Signed USDT amount (e.g. -0.02 → "−$0,02"). */
export function formatUsd(locale: Locale, value: number, digits = 2): string {
  if (!Number.isFinite(value)) return '—';
  const sign = value >= 0 ? '+' : '−';
  return `${sign}$${formatNumber(locale, Math.abs(value), digits)}`;
}

/** Price with a sane decimal count (matches the old fmtPrice heuristic). */
export function formatPrice(locale: Locale, value: number): string {
  if (!Number.isFinite(value)) return '—';
  return formatNumber(locale, value, value >= 100 ? 2 : 4);
}
