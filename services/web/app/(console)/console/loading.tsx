import { DeckSkeleton } from '@/components/console/DeckSkeleton';

/** Fallback de rota do Next. Mesmo esqueleto que a página usa enquanto busca
 *  os dados, para que a transição entre os dois não desloque nada. */
export default function DashboardLoading() {
  return <DeckSkeleton />;
}
