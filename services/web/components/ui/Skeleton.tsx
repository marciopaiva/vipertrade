import { cn } from '@/lib/utils';

/**
 * Bloco de carregamento no formato do conteúdo que vai chegar.
 *
 * Vale mais que um "Carregando…" centralizado porque preserva o layout: a
 * página não salta quando os dados entram, e dá para começar a ler a estrutura
 * antes de ela estar preenchida.
 *
 * `aria-hidden` porque é decoração — quem anuncia o carregamento para leitor de
 * tela é o `aria-busy` da região, não estes blocos.
 */
export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      aria-hidden
      className={cn(
        'animate-pulse rounded-md border border-border bg-card',
        className
      )}
    />
  );
}

/**
 * Envelope de uma região carregando. Marca `aria-busy` para que o leitor de
 * tela saiba que o conteúdo é provisório, com um rótulo dizendo o que vem.
 */
export function SkeletonRegion({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div role="status" aria-busy aria-label={label} className={className}>
      {children}
    </div>
  );
}
