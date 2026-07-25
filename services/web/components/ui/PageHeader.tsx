import { cn } from '@/lib/utils';

/**
 * Cabeçalho de rota do console. Antes deste componente, três telas repetiam a
 * mesma dupla `h1` + `p` classe por classe, e o Command Deck não tinha título
 * nenhum — ao navegar, o topo da página nunca começava no mesmo lugar.
 *
 * O slot `right` recebe o resumo que a tela quer manter ao alcance do olho
 * (PnL do período, contagem de registros, estado da conexão). Fica alinhado
 * pela base do título, então números e texto assentam na mesma linha ótica.
 *
 * `subtitle` é ReactNode porque /system embute um `<code>` no meio da frase.
 */
export function PageHeader({
  title,
  subtitle,
  right,
  className,
}: {
  title: string;
  subtitle?: React.ReactNode;
  right?: React.ReactNode;
  className?: string;
}) {
  return (
    <header
      className={cn(
        'flex flex-wrap items-end justify-between gap-x-6 gap-y-3',
        className
      )}
    >
      <div className="min-w-0">
        <h1 className="font-display text-2xl font-bold tracking-tight text-foreground">
          {title}
        </h1>
        {subtitle && (
          <p className="mt-1 max-w-prose text-sm text-muted-foreground">
            {subtitle}
          </p>
        )}
      </div>
      {right && <div className="shrink-0">{right}</div>}
    </header>
  );
}
