'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { ViperTradeLogo } from './ViperTradeLogo';
import { HealthPill } from './HealthPill';
import LogoutButton from './auth/LogoutButton';
import { DensityToggle } from './console/DensityToggle';
import { LanguageToggle } from './console/LanguageToggle';
import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';

// Only real destinations live in the nav; labels come from the i18n `nav` namespace.
// Strategy folded into the Command Deck (its decision matrix lives there now),
// so it's no longer a top-level destination.
const NAV = [
  { href: '/console', key: 'console' },
  { href: '/trades', key: 'trades' },
  { href: '/analysis', key: 'analysis' },
  { href: '/system', key: 'system' },
] as const;

export function AppHeader() {
  const pathname = usePathname();
  const t = useT('nav');

  return (
    <header className="border-b border-border/50 bg-viper-navy/90 backdrop-blur-sm sticky top-0 z-50">
      <div className="container mx-auto px-4 py-4">
        <div className="flex items-center justify-between">
          <Link href="/console" className="flex items-center gap-3">
            <ViperTradeLogo size="md" showText={true} />
          </Link>
          <nav className="flex items-center gap-4">
            {NAV.map(item => {
              const active = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  aria-current={active ? 'page' : undefined}
                  className={cn(
                    'text-sm transition-colors',
                    active
                      ? 'text-viper-cyan'
                      : 'text-muted-foreground hover:text-viper-cyan'
                  )}
                >
                  {t(item.key)}
                </Link>
              );
            })}
            <button
              type="button"
              onClick={() =>
                window.dispatchEvent(new Event('command-palette:open'))
              }
              title="Command palette"
              className="hidden items-center gap-1 rounded-md border border-border px-2 py-1 font-mono text-[11px] text-muted-foreground transition-colors hover:text-foreground sm:inline-flex"
            >
              ⌘K
            </button>
            <LanguageToggle />
            <DensityToggle />
            <HealthPill className="ml-1" />
            <LogoutButton />
          </nav>
        </div>
      </div>
    </header>
  );
}
