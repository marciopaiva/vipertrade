'use client';

import { cn } from '@/lib/utils';
import { useT, useLocale, formatPrice, formatPct } from '@/lib/i18n';
import { HudFrame } from '@/components/ui/HudFrame';

export interface SwingSymbolDiagnostic {
  symbol: string;
  price: number;
  ema_slow: number | null;
  ema_fast: number | null;
  uptrend: boolean;
  pullback: boolean;
  stop: number | null;
  target: number | null;
  /** Fração do preço, não pontos percentuais. */
  risk_pct: number | null;
  risk_in_range: boolean;
  has_position: boolean;
  candles: number;
  status: string;
}

export interface SwingMatrixData {
  stale?: boolean;
  btc_uptrend?: boolean;
  btc_price?: number | null;
  btc_ema_slow?: number | null;
  timestamp?: string;
  symbols: SwingSymbolDiagnostic[];
}

// Colunas = as regras do checklist, na ordem em que barram. As antigas (RSI,
// %B, ADX, consenso) mediam o pipeline de scalp, desligado em 2026-08-07: a
// tela mostrava onze linhas de indicadores que não decidiam nada e ainda
// alertava "tendência fraca (ADX 14)" como se algo estivesse filtrando por ali.
const GRID =
  'grid grid-cols-[104px_96px_150px_150px_96px_minmax(190px,1fr)] items-center gap-x-3';

/** Ordem de leitura: primeiro o que exige ação, por último o que está parado. */
const PESO: Record<string, number> = {
  setup: 0,
  position_open: 1,
  awaiting_pullback: 2,
  risk_out_of_range: 3,
  no_uptrend: 4,
  macro_blocked: 5,
  insufficient_history: 6,
};

function statusTone(status: string) {
  if (status === 'setup') return 'bg-accent/15 text-accent border-accent/30';
  if (status === 'position_open')
    return 'bg-sky-500/15 text-sky-400 border-sky-500/30';
  if (status === 'awaiting_pullback')
    return 'bg-amber-500/10 text-amber-400/90 border-amber-500/25';
  return 'bg-muted/40 text-muted-foreground border-border';
}

/** Célula de regra: o valor comparado, e se a regra passou. */
function Rule({
  ok,
  left,
  op,
  right,
  muted,
}: {
  ok: boolean;
  left: string;
  op: string;
  right: string;
  muted?: boolean;
}) {
  return (
    <span
      className={cn(
        'font-mono text-2xs',
        muted ? 'text-muted-foreground/50' : ok ? 'text-accent' : 'text-destructive/80'
      )}
    >
      {muted ? '—' : `${left} ${op} ${right}`}
    </span>
  );
}

export function SwingMatrix({ data }: { data?: SwingMatrixData | null }) {
  const t = useT('swing');
  const locale = useLocale();
  const symbols = data?.symbols ?? [];
  const stale = data?.stale ?? false;

  const ordered = [...symbols].sort(
    (a, b) =>
      (PESO[a.status] ?? 9) - (PESO[b.status] ?? 9) ||
      a.symbol.localeCompare(b.symbol)
  );

  const macroOk = data?.btc_uptrend ?? false;

  return (
    <HudFrame title={t('title')}>
      {/* O filtro do BTC precede o checklist inteiro: fechado, nenhum símbolo
          entra. Por isso fica no topo e não como mais uma coluna. */}
      <div
        className={cn(
          'flex flex-wrap items-center gap-x-3 gap-y-1 border-b px-3 py-2 text-2xs',
          macroOk
            ? 'border-border bg-accent/5'
            : 'border-destructive/30 bg-destructive/10'
        )}
      >
        <span className="text-3xs uppercase tracking-[0.15em] text-muted-foreground">
          {t('macroFilter')}
        </span>
        {data?.btc_price != null && data?.btc_ema_slow != null ? (
          <span className="font-mono text-foreground">
            BTC {formatPrice(locale, data.btc_price)} {macroOk ? '>' : '<'} EMA200{' '}
            {formatPrice(locale, data.btc_ema_slow)}
          </span>
        ) : null}
        <span
          className={cn(
            'rounded border px-1.5 py-0.5 font-medium',
            macroOk
              ? 'border-accent/30 bg-accent/15 text-accent'
              : 'border-destructive/30 bg-destructive/15 text-destructive'
          )}
        >
          {macroOk ? t('macroOpen') : t('macroBlocked')}
        </span>
      </div>

      {stale || ordered.length === 0 ? (
        <div className="px-3 py-10 text-center text-sm text-muted-foreground">
          {stale ? t('staleSnapshot') : t('empty')}
        </div>
      ) : (
        <div className="overflow-x-auto">
          <div
            className={cn(
              GRID,
              'border-b border-border px-3 py-2 text-3xs uppercase tracking-[0.15em] text-muted-foreground'
            )}
          >
            <span>{t('colSymbol')}</span>
            <span>{t('colState')}</span>
            <span>{t('colTrend')}</span>
            <span>{t('colPullback')}</span>
            <span>{t('colRisk')}</span>
            <span>{t('colLevels')}</span>
          </div>

          {ordered.map(d => {
            const semDados = d.candles < 200;
            return (
              <div
                key={d.symbol}
                className={cn(
                  GRID,
                  'border-b border-border/50 px-3 py-2 last:border-0'
                )}
              >
                <div className="min-w-0">
                  <div className="truncate text-2xs font-medium text-foreground">
                    {d.symbol}
                  </div>
                  <div className="font-mono text-3xs text-muted-foreground">
                    {formatPrice(locale, d.price)}
                  </div>
                </div>

                <span
                  className={cn(
                    'justify-self-start rounded border px-1.5 py-0.5 text-3xs font-medium',
                    statusTone(d.status)
                  )}
                >
                  {t(`st_${d.status}` as never)}
                </span>

                <Rule
                  ok={d.uptrend}
                  muted={semDados}
                  left={formatPrice(locale, d.price)}
                  op={d.uptrend ? '>' : '<'}
                  right={
                    d.ema_slow != null ? formatPrice(locale, d.ema_slow) : '—'
                  }
                />
                <Rule
                  ok={d.pullback}
                  muted={semDados}
                  left={formatPrice(locale, d.price)}
                  op={d.pullback ? '≤' : '>'}
                  right={
                    d.ema_fast != null ? formatPrice(locale, d.ema_fast) : '—'
                  }
                />

                <span
                  className={cn(
                    'font-mono text-2xs',
                    d.risk_pct == null
                      ? 'text-muted-foreground/50'
                      : d.risk_in_range
                        ? 'text-foreground'
                        : 'text-destructive/80'
                  )}
                >
                  {d.risk_pct != null ? formatPct(locale, d.risk_pct * 100, 2) : '—'}
                </span>

                {/* Stop e alvo que ESTE símbolo teria se entrasse agora — é o
                    que torna a linha acionável em vez de descritiva. */}
                <span className="font-mono text-2xs text-muted-foreground">
                  {d.stop != null && d.target != null ? (
                    <>
                      {t('stopShort')} {formatPrice(locale, d.stop)}
                      <span className="px-1.5 text-muted-foreground/40">·</span>
                      {t('targetShort')} {formatPrice(locale, d.target)}
                    </>
                  ) : (
                    '—'
                  )}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </HudFrame>
  );
}
