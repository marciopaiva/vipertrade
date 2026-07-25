'use client';

import Image from 'next/image';
import { cn } from '@/lib/utils';

interface ViperTradeLogoProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
  showText?: boolean;
}

export function ViperTradeLogo({
  size = 'md',
  className,
  showText = true,
}: ViperTradeLogoProps) {
  const sizeConfig = {
    sm: { width: 32, height: 32 },
    md: { width: 48, height: 48 },
    lg: { width: 96, height: 96 },
  };

  const { width, height } = sizeConfig[size];

  return (
    <div className={cn('flex items-center gap-3', className)}>
      <div className="relative" style={{ width, height }}>
        <Image
          src="/logo.png"
          alt="ViperTrade Logo"
          width={width}
          height={height}
          className="rounded-lg"
          priority
        />
      </div>
      {showText && (
        <div>
          {/* span, não h1: a marca vive no cabeçalho de navegação, e o h1 de
              cada rota é o título da própria página (<PageHeader>). Dois h1 na
              mesma página quebram a árvore de headings para leitor de tela. */}
          <span
            className="block text-xl font-bold text-primary"
            style={{ letterSpacing: '0.5px' }}
          >
            ViperTrade
          </span>
          <p className="text-xs text-muted-foreground">
            Lead Trader Bot - Bybit Copy Trading
          </p>
        </div>
      )}
    </div>
  );
}
