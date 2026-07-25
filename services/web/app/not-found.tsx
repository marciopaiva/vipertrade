'use client';

import Link from 'next/link';
import { useT } from '@/lib/i18n';

export default function NotFound() {
  const t = useT('app');
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background">
      <div className="text-center space-y-6 p-8">
        {/* 404 Text */}
        <h1 className="text-9xl font-bold text-primary opacity-20">404</h1>

        {/* Message */}
        <div className="space-y-2">
          <h2 className="text-2xl font-bold text-foreground">
            {t('notFoundTitle')}
          </h2>
          <p className="text-muted-foreground">{t('notFoundBody')}</p>
        </div>

        {/* Back Button */}
        <Link
          href="/"
          className="inline-block rounded-lg bg-primary px-6 py-2 font-semibold text-primary-foreground transition-colors hover:bg-primary/90"
        >
          {t('goDashboard')}
        </Link>
      </div>
    </div>
  );
}
