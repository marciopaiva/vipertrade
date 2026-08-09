'use client';

import { useEffect, useState } from 'react';
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
  /** Fração positiva do preço até o próximo obstáculo; null se não for preço. */
  distance_pct: number | null;
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
  'grid grid-cols-[104px_96px_150px_150px_84px_150px_minmax(170px,1fr)] items-center gap-x-3';

// Acima disto o símbolo não é acionável no horizonte de dias — a barra satura e
// para de disputar atenção com quem está a menos de 1%.
const DISTANCIA_MAX_PCT = 5;

/** Ordem de leitura: primeiro o que exige ação, por último o que está parado. */
const PESO: Record<string, number> = {
  setup: 0,
  position_open: 1,
  stop_cooldown: 2,
  awaiting_pullback: 3,
  risk_out_of_range: 4,
  no_uptrend: 5,
  macro_blocked: 6,
  insufficient_history: 7,
};

function statusTone(status: string) {
  if (status === 'setup') return 'bg-accent/15 text-accent border-accent/30';
  if (status === 'position_open')
    return 'bg-sky-500/15 text-sky-400 border-sky-500/30';
  if (status === 'stop_cooldown')
    return 'bg-destructive/10 text-destructive/90 border-destructive/25';
  if (status === 'awaiting_pullback')
    return 'bg-amber-500/10 text-amber-400/90 border-amber-500/25';
  return 'bg-muted/40 text-muted-foreground border-border';
}

/**
 * Termômetro de proximidade: quanto FALTA de movimento até o próximo obstáculo.
 *
 * Cheio = colado no gatilho. A escala satura em 5%, senão um símbolo a 12% da
 * EMA200 achataria a barra de quem está a 0,4% da EMA50 — e é justamente esse
 * que o operador precisa enxergar.
 */
function Proximity({
  pct,
  status,
}: {
  pct: number | null;
  status: string;
}) {
  const t = useT('swing');
  const locale = useLocale();
  if (pct == null) {
    return <span className="text-2xs text-muted-foreground/40">—</span>;
  }
  const p = pct * 100;
  const cheio = Math.max(0, Math.min(1, 1 - p / DISTANCIA_MAX_PCT));
  const perto = p <= 1;
  const cor =
    status === 'setup'
      ? 'bg-accent'
      : perto
        ? 'bg-amber-400'
        : cheio > 0.4
          ? 'bg-amber-500/50'
          : 'bg-muted-foreground/30';
  // Seta = para onde o preço precisa ir. Sem ela "falta 5,2%" é ambíguo.
  const seta = status === 'no_uptrend' ? '↑' : status === 'awaiting_pullback' ? '↓' : '';
  return (
    <div className="flex items-center gap-1.5">
      <div className="h-1.5 w-14 shrink-0 overflow-hidden rounded-full bg-muted/50">
        <div
          className={cn('h-full rounded-full transition-all', cor)}
          style={{ width: `${cheio * 100}%` }}
        />
      </div>
      <span
        className={cn(
          'font-mono text-3xs tabular-nums',
          status === 'setup'
            ? 'text-accent'
            : perto
              ? 'text-amber-400'
              : 'text-muted-foreground'
        )}
      >
        {status === 'setup' ? t('atTrigger') : `${seta}${formatPct(locale, p, p < 1 ? 2 : 1)}`}
      </span>
    </div>
  );
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

/**
 * "avaliado há Xs" — a prova de que a matriz está viva.
 *
 * As colunas quase não mudam: o preço só é relido a cada 5 min (a vela é de 4H)
 * e o ESTADO só vira quando o preço cruza uma EMA de 200 períodos de 4H, o que
 * leva horas ou dias. Uma tela correta e uma tela travada são visualmente
 * idênticas sem isto, e já tivemos um caso em que a matriz ficou horas exibindo
 * um 404 como se fosse leitura de mercado.
 */
function Frescor({ iso }: { iso?: string }) {
  const t = useT('swing');
  // Inicializador lazy (não setState dentro do efeito) — mesmo padrão do
  // relógio da página do console.
  const [agora, setAgora] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setAgora(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  if (!iso) return null;
  const ms = agora - new Date(iso).getTime();
  if (!Number.isFinite(ms) || ms < 0) return null;
  const seg = Math.floor(ms / 1000);
  const texto = seg < 90 ? `${seg}s` : `${Math.floor(seg / 60)}min`;
  // O strategy reavalia a cada 60s; passando de 3 min alguma coisa parou.
  const velho = seg > 180;
  return (
    <span
      className={cn(
        'ml-auto flex items-center gap-1.5 font-mono text-3xs',
        velho ? 'text-destructive' : 'text-muted-foreground'
      )}
    >
      <span
        className={cn(
          'inline-block h-1.5 w-1.5 rounded-full',
          velho ? 'bg-destructive' : 'animate-pulse bg-accent'
        )}
      />
      {t('evaluatedAgo', { age: texto })}
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

  // Sem payload não há veredito: `btc_uptrend` ausente virava "BTC bloqueia
  // todas as entradas", que é uma afirmação forte sobre o mercado feita a partir
  // de um erro de rede. Um 404 no proxy ficou horas assim, indistinguível de um
  // filtro macro realmente fechado.
  const semDados = data == null || data.btc_uptrend == null;
  const macroOk = data?.btc_uptrend === true;

  return (
    <HudFrame title={t('title')}>
      {/* O filtro do BTC precede o checklist inteiro: fechado, nenhum símbolo
          entra. Por isso fica no topo e não como mais uma coluna. */}
      <div
        className={cn(
          'flex flex-wrap items-center gap-x-3 gap-y-1 border-b px-3 py-2 text-2xs',
          semDados
            ? 'border-border bg-muted/20'
            : macroOk
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
            semDados
              ? 'border-border bg-muted/40 text-muted-foreground'
              : macroOk
                ? 'border-accent/30 bg-accent/15 text-accent'
                : 'border-destructive/30 bg-destructive/15 text-destructive'
          )}
        >
          {semDados
            ? t('macroUnknown')
            : macroOk
              ? t('macroOpen')
              : t('macroBlocked')}
        </span>
        <Frescor iso={data?.timestamp} />
      </div>

      {stale || semDados || ordered.length === 0 ? (
        <div className="px-3 py-10 text-center text-sm text-muted-foreground">
          {semDados ? t('unavailable') : stale ? t('staleSnapshot') : t('empty')}
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
            <span>{t('colDistance')}</span>
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

                <Proximity pct={d.distance_pct} status={d.status} />

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
