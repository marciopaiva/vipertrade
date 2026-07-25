// services/web/app/error.tsx
'use client';

import { useEffect } from 'react';
import { useT } from '@/lib/i18n';

export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  const t = useT('app');
  useEffect(() => {
    console.error('Application error:', error);
  }, [error]);

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background">
      <div className="text-center space-y-4">
        <h2 className="text-2xl font-bold text-destructive">
          {t('somethingWrong')}
        </h2>
        <p className="text-sm text-muted-foreground">{error.message}</p>
        <button
          onClick={reset}
          className="px-6 py-2 bg-gradient-to-r from-primary to-accent text-primary-foreground font-semibold rounded-lg hover:opacity-90 transition"
        >
          {t('tryAgain')}
        </button>
      </div>
    </div>
  );
}
