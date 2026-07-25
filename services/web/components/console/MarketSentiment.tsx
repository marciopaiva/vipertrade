'use client';

import { useDashboard } from '@/hooks/useDashboard';
import { useT } from '@/lib/i18n';
import { RadialGauge, type GaugeStop } from '@/components/ui/RadialGauge';
import { scale5, scaleStep } from '@/lib/chartColors';

type T = ReturnType<typeof useT<'console'>>;

interface FearGreed {
  value: number;
  classification: string;
  updatedAt: string | null;
}

interface LongShort {
  symbol: string;
  longPct: number;
  shortPct: number;
  updatedAt: string | null;
}

interface SentimentResponse {
  fearGreed: FearGreed | null;
  longShort: LongShort | null;
  errors?: string[];
}

// Fear & Greed gradient (low/red → high/green), shared by the dial arc.
// Escala sequencial do tema (`--scale-1..5`), não hex solto.
const FNG_STOPS: GaugeStop[] = scale5.map((color, i) => ({
  offset: i * 25,
  color,
}));

// Zone color + classification label (note/blurb intentionally dropped — the
// dial + label carry the signal without a wall of text).
function zone(value: number, t: T): { color: string; label: string } {
  const color = scaleStep(value);
  if (value < 25) return { color, label: t('fngExtremeFear') };
  if (value < 45) return { color, label: t('fngFear') };
  if (value < 55) return { color, label: t('fngNeutral') };
  if (value < 75) return { color, label: t('fngGreed') };
  return { color, label: t('fngExtremeGreed') };
}

function LongShortBar({ ls, t }: { ls: LongShort; t: T }) {
  const long = Math.round(ls.longPct);
  const short = Math.round(ls.shortPct);
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>
          {t('longShortRatio')} · {ls.symbol}
        </span>
        <span>{t('bybitPerps')}</span>
      </div>
      <div className="flex h-2.5 overflow-hidden rounded-full bg-secondary">
        <div className="h-full bg-accent" style={{ width: `${ls.longPct}%` }} />
        <div
          className="h-full bg-destructive"
          style={{ width: `${ls.shortPct}%` }}
        />
      </div>
      <div className="flex items-center justify-between text-xs">
        <span className="font-semibold text-accent">
          {long}%{' '}
          <span className="font-normal text-muted-foreground">{t('long')}</span>
        </span>
        <span className="font-semibold text-destructive">
          <span className="font-normal text-muted-foreground">
            {t('short')}
          </span>{' '}
          {short}%
        </span>
      </div>
    </div>
  );
}

export function MarketSentiment() {
  const t = useT('console');
  const tc = useT('common');
  const { data, loading } = useDashboard<SentimentResponse>('/api/sentiment', {
    refreshInterval: 60000,
  });

  const fg = data?.fearGreed ?? null;
  const ls = data?.longShort ?? null;
  const z = fg ? zone(fg.value, t) : null;

  if (loading && !data) {
    return (
      <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
        {tc('loading')}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
      <div className="mx-auto w-full max-w-[240px] shrink-0 sm:mx-0">
        {fg && z ? (
          <RadialGauge
            value={fg.value}
            min={0}
            max={100}
            stops={FNG_STOPS}
            color={z.color}
            sublabel={z.label}
            sweep
            ariaLabel={`${t('fngTitle')} ${fg.value}, ${z.label}`}
          />
        ) : (
          <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
            {t('indexUnavailable')}
          </div>
        )}
      </div>
      <div className="flex-1">
        {ls ? (
          <LongShortBar ls={ls} t={t} />
        ) : (
          <p className="text-xs text-muted-foreground">
            {t('longShortUnavailable')}
          </p>
        )}
      </div>
    </div>
  );
}
