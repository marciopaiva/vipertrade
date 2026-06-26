import type { useT } from '@/lib/i18n';

type T = ReturnType<typeof useT<'trades'>>;

// Known backend close_reason enum values → catalog keys. Anything not listed
// falls back to a title-cased version of the raw value, so a new reason still
// renders readably until a translation is added.
const KNOWN: Record<string, Parameters<T>[0]> = {
  trailing_stop: 'reasonTrailingStop',
  stop_loss: 'reasonStopLoss',
  thesis_invalidated: 'reasonThesisInvalidated',
  take_profit: 'reasonTakeProfit',
  manual: 'reasonManual',
  liquidation: 'reasonLiquidation',
  break_even: 'reasonBreakEven',
};

function titleCase(value: string) {
  return value
    .replaceAll('_', ' ')
    .toLowerCase()
    .replace(/\b\w/g, c => c.toUpperCase());
}

/** Translate a close_reason for display; falls back to title case for unknowns. */
export function reasonLabel(t: T, raw?: string | null): string {
  if (!raw || raw === 'unknown') return t('reasonUnknown');
  const key = KNOWN[raw];
  return key ? t(key) : titleCase(raw);
}
