'use client';

import { useState } from 'react';
import { cn } from '@/lib/utils';
import LiveQualityTab from '@/components/analysis/LiveQualityTab';
import TuningTab from '@/components/analysis/TuningTab';
import { useTuning } from '@/components/analysis/tuningShared';

type TabId = 'live' | 'whatif';

export default function AnalysisPage() {
  const [tab, setTab] = useState<TabId>('live');
  const tuning = useTuning();

  const tabs: Array<{ id: TabId; label: string }> = [
    { id: 'live', label: 'Ao Vivo' },
    { id: 'whatif', label: 'What-if (grid)' },
  ];

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-2xl font-bold tracking-tight text-foreground">Analysis</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Qualidade de operação <strong>ao vivo</strong> (trades realizados) e simulação
          determinística de tuning (<strong>what-if</strong>). Mudanças de config são aplicadas
          manualmente.
        </p>
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
