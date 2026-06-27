'use client';

import { useDashboard } from '@/hooks/useDashboard';
import { useT } from '@/lib/i18n';

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

// Fear & Greed zones, low→high. Color is shared by the arc marker, the big
// number, and the classification label so the whole card reads as one signal.
// Labels/notes resolve through the i18n catalog (pt-BR/en).
function zone(value: number, t: T): { color: string; label: string; note: string } {
  if (value < 25)
    return { color: '#ef4444', label: t('fngExtremeFear'), note: t('fngExtremeFearNote') };
  if (value < 45)
    return { color: '#f97316', label: t('fngFear'), note: t('fngFearNote') };
  if (value < 55)
    return { color: '#eab308', label: t('fngNeutral'), note: t('fngNeutralNote') };
  if (value < 75)
    return { color: '#84cc16', label: t('fngGreed'), note: t('fngGreedNote') };
  return { color: '#22c55e', label: t('fngExtremeGreed'), note: t('fngExtremeGreedNote') };
}

// Point on the top semicircle for a 0–100 value (0 = left/red, 100 = right/green).
function arcPoint(value: number, radius: number, cx: number, cy: number) {
  const f = Math.min(100, Math.max(0, value)) / 100;
  const angle = Math.PI * (1 - f); // π (left) → 0 (right)
  return { x: cx + radius * Math.cos(angle), y: cy - radius * Math.sin(angle) };
}

function Gauge({ fg, t }: { fg: FearGreed; t: T }) {
  const cx = 100;
  const cy = 100;
  const R = 80;
  const z = zone(fg.value, t);
  const marker = arcPoint(fg.value, R, cx, cy);

  // Dotted scale along an inner arc, mirroring the Bybit reference.
  const ticks = Array.from({ length: 11 }, (_, i) =>
    arcPoint((i / 10) * 100, R - 16, cx, cy)
  );

  return (
    <div className="relative">
      <svg
        viewBox="0 0 200 118"
        className="w-full"
        role="img"
        aria-label={`${t('fngTitle')} ${fg.value}, ${z.label}`}
      >
        <defs>
          <linearGradient id="fng-arc" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0%" stopColor="#ef4444" />
            <stop offset="25%" stopColor="#f97316" />
            <stop offset="50%" stopColor="#eab308" />
            <stop offset="75%" stopColor="#84cc16" />
            <stop offset="100%" stopColor="#22c55e" />
          </linearGradient>
        </defs>

        {/* Gradient arc */}
        <path
          d={`M ${cx - R} ${cy} A ${R} ${R} 0 0 1 ${cx + R} ${cy}`}
          fill="none"
          stroke="url(#fng-arc)"
          strokeWidth={10}
          strokeLinecap="round"
        />

        {/* Dotted scale */}
        {ticks.map((t, i) => (
          <circle key={i} cx={t.x} cy={t.y} r={1.6} fill="#475569" />
        ))}

        {/* Needle + arc marker */}
        <line
          x1={cx}
          y1={cy}
          x2={marker.x}
          y2={marker.y}
          stroke={z.color}
          strokeWidth={2}
          strokeLinecap="round"
        />
        <circle cx={marker.x} cy={marker.y} r={9} fill={z.color} opacity={0.25} />
        <circle cx={marker.x} cy={marker.y} r={5} fill={z.color} />
        <circle cx={cx} cy={cy} r={4} fill={z.color} />
      </svg>

      {/* Value + classification, centered under the dome */}
      <div className="-mt-10 flex flex-col items-center">
        <span
          className="text-4xl font-bold leading-none"
          style={{ color: z.color }}
        >
          {fg.value}
        </span>
        <span className="mt-1 text-sm font-medium" style={{ color: z.color }}>
          {z.label}
        </span>
      </div>
    </div>
  );
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
        <div
          className="h-full bg-emerald-500"
          style={{ width: `${ls.longPct}%` }}
        />
        <div className="h-full bg-red-500" style={{ width: `${ls.shortPct}%` }} />
      </div>
      <div className="flex items-center justify-between text-xs">
        <span className="font-semibold text-emerald-500">
          {long}%{' '}
          <span className="font-normal text-muted-foreground">{t('long')}</span>
        </span>
        <span className="font-semibold text-red-500">
          <span className="font-normal text-muted-foreground">{t('short')}</span>{' '}
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

  return (
    <>
      {loading && !data ? (
        <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
          {tc('loading')}
        </div>
      ) : (
        <div className="flex flex-col gap-6 md:flex-row md:items-center">
          {/* Gauge — fixed lane on the left so it doesn't stretch full width. */}
          <div className="mx-auto w-full max-w-[260px] shrink-0 md:mx-0">
            {fg ? (
              <Gauge fg={fg} t={t} />
            ) : (
              <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
                {t('indexUnavailable')}
              </div>
            )}
          </div>

          {/* Explanation + long/short ratio fill the rest of the row. */}
          <div className="flex-1 space-y-4">
            {fg && z && (
              <div className="space-y-1">
                <p className="text-base font-semibold" style={{ color: z.color }}>
                  {z.label} · {fg.value}/100
                </p>
                <p className="text-sm text-muted-foreground">
                  {t('fngBlurb')} {z.note}
                </p>
              </div>
            )}
            {ls ? (
              <LongShortBar ls={ls} t={t} />
            ) : (
              <p className="text-xs text-muted-foreground">
                {t('longShortUnavailable')}
              </p>
            )}
          </div>
        </div>
      )}
    </>
  );
}
