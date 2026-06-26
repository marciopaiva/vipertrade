'use client';

import { useState } from 'react';
import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';
import LiveQualityTab from '@/components/analysis/LiveQualityTab';
import TuningTab from '@/components/analysis/TuningTab';
import { useTuning } from '@/components/analysis/tuningShared';

type TabId = 'live' | 'whatif';

export default function AnalysisPage() {
  const [tab, setTab] = useState<TabId>('live');
  const tuning = useTuning();
  const t = useT('analysis');

  const tabs: Array<{ id: TabId; label: string }> = [
    { id: 'live', label: t('tabLive') },
    { id: 'whatif', label: t('tabWhatif') },
  ];

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-foreground">{t('title')}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{t('subtitle')}</p>
      </div>

      <div className="flex gap-1 border-b border-border">
        {tabs.map(t => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            className={cn(
              '-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors',
              tab === t.id
                ? 'border-accent text-foreground'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'live' && <LiveQualityTab />}
      {tab === 'whatif' && <TuningTab tuning={tuning} />}
    </div>
  );
}
