'use client';

import Link from 'next/link';
import { ViperTradeLogo } from './ViperTradeLogo';
import { HealthPill } from './HealthPill';
import LogoutButton from './auth/LogoutButton';

export function AppHeader() {
  return (
    <header className="border-b border-border/50 bg-viper-navy/90 backdrop-blur-sm sticky top-0 z-50">
      <div className="container mx-auto px-4 py-4">
        <div className="flex items-center justify-between">
          <Link href="/dashboard" className="flex items-center gap-3">
            <ViperTradeLogo size="md" showText={true} />
          </Link>
          <nav className="flex items-center gap-4">
            {/* Only real destinations live in the nav; the Trades/Positions/
                Settings stubs were demoted until they ship (see
                docs/design/web-experience-proposal.md). */}
            <Link href="/dashboard" className="text-sm text-muted-foreground hover:text-viper-cyan">
              Dashboard
            </Link>
            <Link href="/analysis" className="text-sm text-muted-foreground hover:text-viper-cyan">
              Analysis
            </Link>
            <HealthPill className="ml-1" />
            <LogoutButton />
          </nav>
        </div>
      </div>
    </header>
  );
}
