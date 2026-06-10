'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { cn } from '@/lib/utils';

interface Trade {
  trade_id: string;
  symbol: string;
  side: string;
  status: string;
  pnl?: number;
  close_reason?: string;
  opened_at: string;
  closed_at?: string;
}

const MAX_ROWS = 8;
const GLOW_MS = 1400;

function titleCase(value?: string) {
  if (!value) return 'closed';
  return value
    .replaceAll('_', ' ')
    .toLowerCase()
    .replace(/\b\w/g, c => c.toUpperCase());
}

function relTime(closedAt?: string, now = Date.now()) {
  if (!closedAt) return '—';
  const ts = Date.parse(closedAt);
  if (!Number.isFinite(ts)) return '—';
  const s = Math.max(0, Math.floor((now - ts) / 1000));
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  return `${h}h ago`;
}

/**
 * Live feed of the most recent fills (closed trades). New rows glow in once on
 * arrival so the operator notices a fill without watching — calm baseline, loud
 * on signal. Data arrives via the dashboard poll; freshly-seen trade_ids glow,
 * the initial batch does not.
 */
export function RecentFills({ trades }: { trades: Trade[] }) {
  const [glowing, setGlowing] = useState<Set<string>>(new Set());
  const [now, setNow] = useState(() => Date.now());
  const seen = useRef<Set<string> | null>(null);

  const recent = useMemo(
    () =>
      trades
        .filter(t => t.status === 'closed')
        .sort(
          (a, b) =>
            Date.parse(b.closed_at || b.opened_at) -
            Date.parse(a.closed_at || a.opened_at)
        )
        .slice(0, MAX_ROWS),
    [trades]
  );

  const idsKey = recent.map(t => t.trade_id).join(',');

  // Detect newly-arrived fills and glow them once. The first batch seeds the
  // "seen" set without glowing so a page load isn't a wall of animation.
  useEffect(() => {
    const ids = recent.map(t => t.trade_id);
    if (seen.current === null) {
      seen.current = new Set(ids);
      return;
    }
    const fresh = ids.filter(id => !seen.current!.has(id));
    ids.forEach(id => seen.current!.add(id));
    if (fresh.length === 0) return;
    setGlowing(prev => new Set([...prev, ...fresh]));
    const timer = setTimeout(() => {
      setGlowing(prev => {
        const next = new Set(prev);
        fresh.forEach(id => next.delete(id));
        return next;
      });
    }, GLOW_MS);
    return () => clearTimeout(timer);
  }, [idsKey, recent]);

  // Keep "Xs/Xm ago" honest without a full data refresh.
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 15_000);
    return () => clearInterval(id);
  }, []);

  return (
    <section>
      <h2 className="mb-2 text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
        Recent fills
      </h2>
      {recent.length === 0 ? (
        <div className="rounded-xl border border-border bg-card px-3 py-8 text-center text-sm text-muted-foreground">
          No fills yet — closed trades stream in here as they happen.
        </div>
      ) : (
        <div className="overflow-hidden rounded-xl border border-border bg-card">
          {recent.map(t => {
            const pnl = t.pnl ?? 0;
            const win = pnl >= 0;
            const isLong = t.side.toLowerCase() === 'long';
            return (
              <div
                key={t.trade_id}
                className={cn(
                  'flex items-center gap-3 border-b border-border/50 px-4 py-2.5 text-sm transition-colors duration-700 last:border-b-0',
                  glowing.has(t.trade_id) && 'bg-accent/10'
                )}
              >
                <span className="w-24 truncate font-semibold text-foreground">
                  {t.symbol}
                </span>
                <span
                  className={cn(
                    'w-12 text-xs font-semibold uppercase',
                    isLong ? 'text-accent' : 'text-destructive'
                  )}
                >
                  {isLong ? 'Long' : 'Short'}
                </span>
                <span
                  className={cn(
                    'w-20 text-right font-mono tabular-nums',
                    win ? 'text-accent' : 'text-destructive'
                  )}
                >
                  {win ? '+' : '−'}${Math.abs(pnl).toFixed(2)}
                </span>
                <span className="flex-1 truncate text-xs text-muted-foreground">
                  {titleCase(t.close_reason)}
                </span>
                <span className="shrink-0 font-mono text-xs tabular-nums text-muted-foreground">
                  {relTime(t.closed_at, now)}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
