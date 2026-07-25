'use client';

import { useEffect, useRef, useState } from 'react';
import { User } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';
import { LanguageToggle } from './LanguageToggle';
import { DensityToggle } from './DensityToggle';
import LogoutButton from '../auth/LogoutButton';

/**
 * Preferências e conta, recolhidas atrás de um botão no cabeçalho.
 *
 * Idioma, densidade e sair ficavam soltos ao lado dos links de navegação, com o
 * mesmo peso visual deles — "Sair" pesava tanto quanto "Command Deck". São
 * ajustes que se mexem uma vez e não se olha mais, então saem do caminho e
 * deixam à vista só o que se consulta em operação: navegação, busca e saúde.
 *
 * Menu próprio em vez de trazer @radix-ui/react-dropdown-menu: são três itens,
 * e o projeto só tem o `react-slot` do Radix instalado.
 */
export function OperatorMenu({ className }: { className?: string }) {
  const t = useT('common');
  const [open, setOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;

    function onPointerDown(e: MouseEvent | TouchEvent) {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        setOpen(false);
        buttonRef.current?.focus(); // devolve o foco a quem abriu
      }
    }

    document.addEventListener('mousedown', onPointerDown);
    document.addEventListener('touchstart', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
      document.removeEventListener('touchstart', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [open]);

  return (
    <div ref={wrapRef} className={cn('relative', className)}>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setOpen(o => !o)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t('accountMenu')}
        className={cn(
          'flex h-7 w-7 items-center justify-center rounded-full border transition-colors',
          open
            ? 'border-primary/50 bg-secondary text-foreground'
            : 'border-border text-muted-foreground hover:border-primary/40 hover:text-foreground'
        )}
      >
        <User aria-hidden className="h-3.5 w-3.5" />
      </button>

      {open && (
        <div
          role="menu"
          aria-label={t('accountMenu')}
          className="absolute right-0 top-full z-50 mt-2 w-56 rounded-md border border-border bg-card p-1 shadow-lg"
        >
          <div className="flex items-center justify-between gap-3 rounded px-2 py-1.5">
            <span className="text-xs text-muted-foreground">
              {t('language')}
            </span>
            <LanguageToggle />
          </div>

          <div className="flex items-center justify-between gap-3 rounded px-2 py-1.5">
            <span className="text-xs text-muted-foreground">
              {t('density')}
            </span>
            <DensityToggle />
          </div>

          <div className="my-1 border-t border-border" />

          <div className="px-2 py-1.5">
            <LogoutButton />
          </div>
        </div>
      )}
    </div>
  );
}
