'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { useLocale, useT, formatSigned, formatNumber } from '@/lib/i18n';
import { Kpi } from '@/components/ui/Kpi';
import { SectionCard } from '@/components/ui/SectionCard';
import {
  ClassBadge,
  GridVariant,
  SubstitutionCard,
  tone,
  TuningState,
} from '@/components/analysis/tuningShared';

function DiffBlock({ variant }: { variant: GridVariant }) {
  const [copied, setCopied] = useState(false);
  const t = useT('common');
  const leaf = variant.path.split('.').pop() ?? variant.path;
  const snippet = `# config/trading/pairs.yaml → global.${variant.path}\n${leaf}: ${variant.value}`;
  return (
    <div className="relative mt-3 rounded-lg border border-border bg-background/60 p-3">
      <button
        type="button"
        onClick={() => {
          void navigator.clipboard.writeText(snippet);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
        className="absolute right-2 top-2 rounded-md border border-border bg-secondary/40 px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground hover:text-foreground"
      >
        {copied ? t('copied') : t('copy')}
      </button>
      <pre className="overflow-x-auto whitespace-pre-wrap font-mono text-xs text-foreground/90">
        {snippet}
      </pre>
    </div>
  );
}

function RecommendationCard({ rec }: { rec: GridVariant | null }) {
  const t = useT('whatif');
  const locale = useLocale();
  return (
    <SectionCard title={t('recTitle')} tone="accent">
      {rec ? (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-sm text-foreground">{rec.path}</span>
            <span className="font-mono text-sm font-semibold text-foreground">= {rec.value}</span>
            <ClassBadge klass={rec.class} />
            <span className={cn('font-mono text-sm tabular-nums', tone(rec.delta_net_pnl))}>
              Δ {formatSigned(locale, rec.delta_net_pnl, 4)}
            </span>
          </div>
          <p className="mt-1.5 text-xs text-muted-foreground">{t('recBest')}</p>
          <DiffBlock variant={rec} />
        </>
      ) : (
        <p className="text-sm text-muted-foreground">{t('recNone')}</p>
      )}
    </SectionCard>
  );
}

function GridTable({ variants }: { variants: GridVariant[] }) {
  const t = useT('whatif');
  const locale = useLocale();
  return (
    <SectionCard title={t('gridTitle')}>
      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-[10px] uppercase tracking-wider text-muted-foreground">
              <th className="pb-2 pr-3 font-medium">{t('colAxis')}</th>
              <th className="pb-2 pr-3 font-medium">{t('colValue')}</th>
              <th className="pb-2 pr-3 font-medium">{t('colClass')}</th>
              <th className="pb-2 pr-3 text-right font-medium">{t('colDelta')}</th>
              <th className="pb-2 pr-3 text-right font-medium">{t('colNet')}</th>
              <th className="pb-2 text-right font-medium">{t('colWL')}</th>
            </tr>
          </thead>
          <tbody className="font-mono tabular-nums">
            {variants.map(v => (
              <tr key={`${v.path}-${v.value}`} className="border-t border-border/50">
                <td className="py-1.5 pr-3 text-foreground/80">{v.axis}</td>
                <td className="py-1.5 pr-3 text-foreground">{v.value}</td>
                <td className="py-1.5 pr-3">
                  <ClassBadge klass={v.class} />
                </td>
                <td className={cn('py-1.5 pr-3 text-right font-semibold', tone(v.delta_net_pnl))}>
                  {formatSigned(locale, v.delta_net_pnl, 4)}
                </td>
                <td className={cn('py-1.5 pr-3 text-right', tone(v.net_pnl))}>
                  {formatSigned(locale, v.net_pnl, 4)}
                </td>
                <td className="py-1.5 text-right text-muted-foreground">
                  {v.wins}/{v.losses}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </SectionCard>
  );
}

function RunBar({ tuning }: { tuning: TuningState }) {
  const tw = useT('whatif');
  const tc = useT('common');
  return (
    <section className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-card p-5">
      <p className="max-w-2xl text-xs text-muted-foreground">{tw('blurb')}</p>
      <Button onClick={() => void tuning.run()} disabled={tuning.loading}>
        {tuning.loading ? tc('running') : tuning.data ? tc('regenerate') : tc('generate')}
      </Button>
    </section>
  );
}

export default function TuningTab({ tuning }: { tuning: TuningState }) {
  const { data, loading, error } = tuning;
  const t = useT('whatif');
  const locale = useLocale();
  return (
    <div className="space-y-5">
      <RunBar tuning={tuning} />

      {error && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {loading && <div className="h-40 animate-pulse rounded-xl border border-border bg-card" />}

      {data && !loading && (
        <>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Kpi label={t('kpiCorpus')} value={formatNumber(locale, data.corpus_ticks, 0)} />
            <Kpi
              label={t('kpiBaseline')}
              value={formatSigned(locale, data.baseline.net_pnl, 4)}
              tone={tone(data.baseline.net_pnl)}
            />
            <Kpi label={t('kpiWin')} value={`${formatNumber(locale, data.baseline.win_rate_pct, 1)}%`} />
            <Kpi label={t('kpiClosed')} value={String(data.baseline.closed)} />
          </div>

          <RecommendationCard rec={data.recommended} />
          <GridTable variants={data.variants} />
          <SubstitutionCard sub={data.substitution} />
        </>
      )}

      {!data && !loading && !error && (
        <div className="flex h-40 items-center justify-center rounded-xl border border-dashed border-border bg-card text-sm text-muted-foreground">
          {t('empty')}
        </div>
      )}
    </div>
  );
}
