'use client';

import { useDashboard } from '@/hooks/useDashboard';
import { useT, useLocale, formatUsd } from '@/lib/i18n';
import { reasonLabel } from '@/components/trades/reasonLabel';
import type { Trade } from '@/types/trading';

/**
 * Live fills ticker for the Command Deck — the most recent closed trades with
 * time · symbol · direction · close-reason · PnL. Pulls from the same
 * `/api/v1/trades` feed as EquityCurve; refreshes every 10s.
 */
export function LiveFeed() {
  const t = useT('deck');
  const tr = useT('trades');
  const locale = useLocale();
  const { data } = useDashboard<{ items: Trade[] }>('/api/v1/trades?limit=40', {
    refreshInterval: 10000,
  });

  const fills = (data?.items ?? [])
    .filter(x => x.status === 'closed')
    .sort(
      (a, b) =>
        Date.parse(b.closed_at || b.opened_at) -
        Date.parse(a.closed_at || a.opened_at)
    )
    .slice(0, 8);

  if (fills.length === 0) {
    return <p className="text-sm text-muted-foreground">{t('feedEmpty')}</p>;
  }

  return (
    <ul className="space-y-1.5 font-mono text-xs tabular-nums">
      {fills.map(f => {
        const win = (f.pnl ?? 0) >= 0;
        const d = new Date(f.closed_at || f.opened_at);
        const time = Number.isNaN(d.getTime())
          ? '—'
          : d.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });
        return (
          <li key={f.trade_id} className="flex items-center justify-between gap-2">
            <span className="shrink-0 text-muted-foreground">{time}</span>
            <span className="flex-1 truncate">
              <span className="font-semibold text-foreground">{f.symbol}</span>{' '}
              <span className={win ? 'text-accent' : 'text-destructive'}>
                {win ? '▲' : '▼'}
              </span>{' '}
              <span className="text-muted-foreground">
                {reasonLabel(tr, f.close_reason)}
              </span>
            </span>
            <span
              className={
                win
                  ? 'shrink-0 text-accent hud-glow-accent'
                  : 'shrink-0 text-destructive hud-glow-danger'
              }
            >
              {formatUsd(locale, f.pnl ?? 0)}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
