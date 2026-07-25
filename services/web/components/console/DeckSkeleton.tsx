'use client';

import { useT } from '@/lib/i18n';
import { Skeleton, SkeletonRegion } from '@/components/ui/Skeleton';

/**
 * Esqueleto do Command Deck, no formato do layout real: cabeçalho, o par
 * patrimônio + sentimento, a barra de indicadores, curva + feed, risco aberto
 * e a matriz de decisão.
 *
 * Um componente só, usado pelo `loading.tsx` da rota e pelo estado de
 * carregamento da própria página — se os dois divergirem, a tela salta na
 * troca. O anterior ainda desenhava seções que a página não tem mais.
 */
export function DeckSkeleton() {
  const tc = useT('console');

  return (
    <SkeletonRegion label={tc('connecting')} className="space-y-4">
      <div className="flex items-end justify-between gap-4">
        <div className="space-y-2">
          <Skeleton className="h-7 w-52" />
          <Skeleton className="h-4 w-80 max-w-full" />
        </div>
        <Skeleton className="h-4 w-28" />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Skeleton className="h-32" />
        <Skeleton className="h-32" />
      </div>

      <Skeleton className="h-16" />

      <div className="grid gap-4 lg:grid-cols-3">
        <Skeleton className="h-64 lg:col-span-2" />
        <Skeleton className="h-64" />
      </div>

      <Skeleton className="h-40" />
    </SkeletonRegion>
  );
}
