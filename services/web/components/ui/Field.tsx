import { cn } from '@/lib/utils';

/**
 * Controles de filtro do console.
 *
 * `/trades` era a única tela que montava `input` e `select` na mão, repetindo
 * as mesmas nove classes em cada um — e foi por isso que a borda de foco e o
 * raio de canto começaram a divergir do resto da interface. Aqui o estilo do
 * controle mora num lugar só.
 */
const CONTROL =
  'rounded-md border border-border bg-card px-2 py-1 text-foreground ' +
  'outline-none transition-colors focus:border-primary/50';

const LABEL = 'flex items-center gap-2 text-xs text-muted-foreground';
const LABEL_TEXT = 'uppercase tracking-wide';

export function SelectField({
  label,
  value,
  onChange,
  options,
  className,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  className?: string;
}) {
  return (
    <label className={cn(LABEL, className)}>
      <span className={LABEL_TEXT}>{label}</span>
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        className={CONTROL}
      >
        {options.map(o => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

/**
 * Botão de filtro rápido ao lado dos campos (atalhos de período, limpar).
 * `active` marca o chip que representa um filtro em vigor — daí ele ganhar o
 * primary, para diferenciar "aplicar" de "aplicado".
 */
export function FilterChip({
  onClick,
  active = false,
  children,
  className,
}: {
  onClick: () => void;
  active?: boolean;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'rounded-md border px-2 py-1 text-xs transition-colors',
        active
          ? 'border-primary/40 bg-primary/10 text-primary hover:bg-primary/15'
          : 'border-border bg-card text-muted-foreground hover:border-primary/40 hover:text-foreground',
        className
      )}
    >
      {children}
    </button>
  );
}

export function DateField({
  label,
  value,
  onChange,
  max,
  className,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  max?: string;
  className?: string;
}) {
  return (
    <label className={cn(LABEL, className)}>
      <span className={LABEL_TEXT}>{label}</span>
      <input
        type="date"
        value={value}
        max={max}
        onChange={e => onChange(e.target.value)}
        // color-scheme:dark faz o seletor nativo do browser vir escuro também
        className={cn(CONTROL, '[color-scheme:dark]')}
      />
    </label>
  );
}
