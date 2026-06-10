'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { ViperTradeLogo } from './ViperTradeLogo';
import { HealthPill } from './HealthPill';
import LogoutButton from './auth/LogoutButton';
import { cn } from '@/lib/utils';

// Only real destinations live in the nav; the Trades/System screens join as they
// ship (see docs/design/web-experience-proposal.md).
const NAV = [
  { href: '/console', label: 'Console' },
  { href: '/strategy', label: 'Strategy' },
  { href: '/analysis', label: 'Analysis' },
];

export function AppHeader() {
  const pathname = usePathname();

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
                  {item.label}
                </Link>
              );
            })}
            <HealthPill className="ml-1" />
            <LogoutButton />
          </nav>
        </div>
      </div>
    </header>
  );
}
