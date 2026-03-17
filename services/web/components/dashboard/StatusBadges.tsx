'use client';

import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

interface MetricCardProps {
  label: string;
  value: string | number;
  accent?: string;
  helper?: string;
  className?: string;
}

export function MetricCard({ label, value, accent = '#11c4ff', helper, className }: MetricCardProps) {
  return (
    <Card className={cn('border-opacity-50', className)}>
      <CardHeader className="pb-2">
        <div className="text-xs uppercase tracking-wider text-muted-foreground">
          {label}
        </div>
      </CardHeader>
      <CardContent>
        <div 
          className="text-2xl font-bold"
          style={{ color: accent }}
        >
          {value}
        </div>
        {helper && (
          <div className="text-xs text-muted-foreground mt-1">
            {helper}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

interface StatusBadgeProps {
  mode: 'paper' | 'testnet' | 'mainnet';
  className?: string;
}

export function TradingModeBadge({ mode, className }: StatusBadgeProps) {
  const config = {
    paper: {
      label: 'PAPER MODE',
      variant: 'success' as const,
    },
    testnet: {
      label: 'TESTNET MODE',
      variant: 'warning' as const,
    },
    mainnet: {
      label: 'MAINNET MODE',
      variant: 'destructive' as const,
    },
  };

  return (
    <Badge variant={config[mode].variant} className={className}>
      {config[mode].label}
    </Badge>
  );
}

interface ExecutorStatusBadgeProps {
  enabled: boolean;
  state?: 'running' | 'paused' | 'down';
  className?: string;
}

export function ExecutorStatusBadge({ enabled, state = 'down', className }: ExecutorStatusBadgeProps) {
  const config = {
    running: {
      label: 'RUNNING',
      variant: 'success' as const,
    },
    paused: {
      label: 'PAUSED',
      variant: 'warning' as const,
    },
    down: {
      label: 'DOWN',
      variant: 'destructive' as const,
    },
  };

  return (
    <Badge variant={config[state].variant} className={className}>
      Executor {config[state].label}
    </Badge>
  );
}

interface KillSwitchBadgeProps {
  enabled: boolean;
  className?: string;
}

export function KillSwitchBadge({ enabled, className }: KillSwitchBadgeProps) {
  return (
    <Badge 
      variant={enabled ? 'destructive' : 'success'} 
      className={className}
    >
      Kill Switch {enabled ? 'ON' : 'OFF'}
    </Badge>
  );
}
