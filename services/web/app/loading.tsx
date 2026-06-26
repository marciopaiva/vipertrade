// services/web/app/loading.tsx
'use client';

import { useT } from '@/lib/i18n';

export default function Loading() {
  const t = useT('app');
  return (
    <div className="flex items-center justify-center min-h-screen bg-[#0a1929]">
      <div className="flex flex-col items-center gap-4">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-cyan-400" />
        <p className="text-cyan-400 text-sm animate-pulse">{t('loadingApp')}</p>
      </div>
    </div>
  );
}
