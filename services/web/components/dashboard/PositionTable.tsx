'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

interface MarketSignal {
  symbol: string;
  current_price: number;
  bybit_price?: number;
  consensus_side?: string;
  bybit_regime?: string;
}

interface Position {
  trade_id: string;
  symbol: string;
  side: string;
  quantity: number;
  notional_usdt: number;
  entry_price: number;
  stop_loss_price?: number | null;
  leverage?: number;
  trailing_stop_activated?: boolean;
  trailing_stop_peak_price?: number | null;
  trailing_stop_final_distance_pct?: number | null;
  trailing_activation_price?: number | null;
  fixed_take_profit_price?: number | null;
  break_even_price?: number | null;
  opened_at?: string;
}

interface PositionTableProps {
  positions: Position[];
  marketSignals?: MarketSignal[];
  className?: string;
}

export function PositionTable({ positions, marketSignals = [], className }: PositionTableProps) {
  const regimeIcon = (regime?: string) => {
    switch ((regime || 'neutral').toLowerCase()) {
      case 'bullish':
        return '↗';
      case 'bearish':
        return '↘';
      default:
        return '•';
    }
  };

  const regimeBadgeStyle = (regime?: string) => {
    switch ((regime || 'neutral').toLowerCase()) {
      case 'bullish':
        return {
          backgroundColor: '#10b98122',
          color: '#34d399',
          borderColor: '#10b98155',
        };
      case 'bearish':
        return {
          backgroundColor: '#ef444422',
          color: '#f87171',
          borderColor: '#ef444455',
        };
      default:
        return {
          backgroundColor: '#ffffff10',
          color: '#f8fafc',
          borderColor: '#ffffff24',
        };
    }
  };

  const formatRelativeOpenTime = (openedAt?: string) => {
    if (!openedAt) return '-';
    const openedMs = Date.parse(openedAt);
    if (Number.isNaN(openedMs)) return '-';
    const diffSeconds = Math.max(0, Math.floor((Date.now() - openedMs) / 1000));
    const hours = Math.floor(diffSeconds / 3600);
    const minutes = Math.floor((diffSeconds % 3600) / 60);

    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m`;
    return '<1m';
  };

  if (positions.length === 0) {
    return (
      <Card className={cn('bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50', className)}>
        <CardHeader className="pb-2">
          <CardTitle className="text-lg text-slate-200">Open Positions</CardTitle>
        </CardHeader>
        <CardContent className="pt-0">
          <div className="text-center text-slate-500 py-8">
            No open positions
          </div>
        </CardContent>
      </Card>
    );
  }

  // Build market price map
  const marketBySymbol = new Map(marketSignals.map(s => [s.symbol, s]));

  // Enrich positions with market data
  const enrichedPositions = positions.map(p => {
    const entryPrice = p.entry_price || (p.quantity > 0 ? p.notional_usdt / p.quantity : 0);
    const signal = marketBySymbol.get(p.symbol);
    const markPrice = signal?.bybit_price ?? signal?.current_price ?? null;
    const isLong = p.side.toLowerCase() === 'long';

    // Calculate unrealized PnL
    const unrealizedPnl = typeof markPrice === 'number'
      ? (isLong ? markPrice - entryPrice : entryPrice - markPrice) * p.quantity
      : null;
    const unrealizedPnlPct = p.notional_usdt > 0 && typeof unrealizedPnl === 'number'
      ? unrealizedPnl / p.notional_usdt
      : null;
    const markDeltaPct = typeof markPrice === 'number' && entryPrice > 0
      ? (isLong ? (markPrice - entryPrice) / entryPrice : (entryPrice - markPrice) / entryPrice)
      : null;

    // Calculate trailing stop info
    const trailingLivePrice = p.trailing_stop_activated &&
      typeof p.trailing_stop_peak_price === 'number' &&
      typeof p.trailing_stop_final_distance_pct === 'number'
      ? isLong
        ? p.trailing_stop_peak_price * (1 - p.trailing_stop_final_distance_pct)
        : p.trailing_stop_peak_price * (1 + p.trailing_stop_final_distance_pct)
      : null;

    const trailingArmedByPrice = !p.trailing_stop_activated &&
      typeof markPrice === 'number' &&
      typeof p.trailing_activation_price === 'number'
      ? isLong
        ? markPrice >= p.trailing_activation_price
        : markPrice <= p.trailing_activation_price
      : false;

    const activationMovePct = entryPrice > 0 && typeof p.trailing_activation_price === 'number'
      ? Math.abs((p.trailing_activation_price - entryPrice) / entryPrice)
      : null;

    const trailingProgressPct = typeof markDeltaPct === 'number' && typeof activationMovePct === 'number' && activationMovePct > 0
      ? Math.max(0, Math.min(1, markDeltaPct / activationMovePct))
      : null;

    const trailingState = p.trailing_stop_activated
      ? { label: 'Live', tone: 'active' as const }
      : trailingArmedByPrice
        ? { label: 'Armed', tone: 'armed' as const }
        : { label: 'Arming', tone: 'waiting' as const };

    const trailingDisplayPrice = typeof trailingLivePrice === 'number'
      ? trailingLivePrice
      : typeof p.trailing_activation_price === 'number'
        ? p.trailing_activation_price
        : typeof p.break_even_price === 'number' && trailingArmedByPrice
          ? p.break_even_price
          : null;

    const distanceToArmPct = !p.trailing_stop_activated &&
      !trailingArmedByPrice &&
      typeof markPrice === 'number' &&
      typeof p.trailing_activation_price === 'number' &&
      entryPrice > 0
      ? Math.abs((p.trailing_activation_price - markPrice) / entryPrice)
      : 0;

    const triggerState = p.trailing_stop_activated && typeof trailingLivePrice === 'number'
      ? `Trail ${trailingLivePrice.toFixed(6)}`
      : typeof p.trailing_activation_price === 'number'
        ? `Arm ${p.trailing_activation_price.toFixed(6)}`
        : typeof p.fixed_take_profit_price === 'number'
          ? `TP ${p.fixed_take_profit_price.toFixed(6)}`
          : '-';

    return {
      ...p,
      entryPrice,
      markPrice,
      unrealizedPnl,
      unrealizedPnlPct,
      markDeltaPct,
      trailingLivePrice,
      trailingArmedByPrice,
      trailingProgressPct,
      trailingState,
      triggerState,
      trailingLabel: typeof trailingLivePrice === 'number'
        ? 'Trailing Arm'
        : typeof p.trailing_activation_price === 'number'
          ? 'Trailing Arm'
          : typeof p.fixed_take_profit_price === 'number'
            ? 'Take Profit'
            : 'Trigger',
      trailingDisplayPrice,
      distanceToArmPct,
      consensusSide: signal?.consensus_side ?? 'neutral',
      bybitRegime: signal?.bybit_regime ?? 'neutral',
      openTimeLabel: formatRelativeOpenTime(p.opened_at),
    };
  });

  return (
    <Card className={cn('bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50', className)}>
      <CardHeader className="pb-2">
        <CardTitle className="text-lg text-slate-200">Open Positions ({positions.length})</CardTitle>
      </CardHeader>
      <CardContent className="pt-0">
        <div className="hidden xl:grid xl:grid-cols-[220px_70px_110px_105px_105px_105px_105px_110px_1fr] gap-4 px-3 pb-2 text-[11px] uppercase tracking-[0.18em] text-slate-500">
          <div>Asset</div>
          <div>Side</div>
          <div className="text-right">PnL</div>
          <div>Entry</div>
          <div>Mark</div>
          <div>Trail</div>
          <div>Stop</div>
          <div>State</div>
          <div>Context</div>
        </div>
        <div className="space-y-2">
          {enrichedPositions.map((position) => {
            const pnlColor = (position.unrealizedPnl ?? 0) >= 0 ? '#10b981' : '#ef4444';
            const sideColor = position.side.toLowerCase() === 'long' ? '#10b981' : '#ef4444';

            return (
              <div
                key={position.trade_id}
                className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-3"
              >
                <div className="grid grid-cols-1 gap-4 xl:grid-cols-[220px_70px_110px_105px_105px_105px_105px_110px_1fr] xl:items-center">
                  <div className="min-w-0">
                    <div className="text-sm font-bold text-slate-200">{position.symbol}</div>
                  </div>

                  <div>
                    <Badge
                      style={{ backgroundColor: sideColor + '22', color: sideColor, borderColor: sideColor + '55' }}
                      className="h-5 min-w-[58px] justify-center px-1.5 py-0.5 text-[10px]"
                    >
                      {position.side.toUpperCase()}
                    </Badge>
                  </div>

                  <div className="text-right xl:pr-2">
                    <div className="text-sm font-bold" style={{ color: pnlColor }}>
                      {position.unrealizedPnl !== null ? `$${position.unrealizedPnl.toFixed(2)}` : '-'}
                    </div>
                    <div className="text-xs" style={{ color: pnlColor }}>
                      {position.unrealizedPnlPct !== null ? `${(position.unrealizedPnlPct! * 100).toFixed(2)}%` : '-'}
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-2 text-xs md:grid-cols-4 xl:contents">
                    <Badge
                      className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                      style={{
                        backgroundColor: '#0f172acc',
                        color: '#e2e8f0',
                        borderColor: '#334155',
                      }}
                    >
                      ${position.entryPrice.toFixed(6)}
                    </Badge>
                    {position.markPrice ? (
                      <Badge
                        className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                        style={{
                          backgroundColor: '#0f172acc',
                          color: '#e2e8f0',
                          borderColor: '#334155',
                        }}
                      >
                        ${position.markPrice.toFixed(6)}
                      </Badge>
                    ) : (
                      <div className="flex h-6 items-center justify-center text-slate-300">-</div>
                    )}
                    {typeof position.trailingDisplayPrice === 'number' ? (
                      <Badge
                        className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                        style={{
                          backgroundColor: position.trailing_stop_activated ? '#10b98118' : position.trailingArmedByPrice ? '#f59e0b14' : '#0f172acc',
                          color: position.trailing_stop_activated ? '#86efac' : position.trailingArmedByPrice ? '#fcd34d' : '#e2e8f0',
                          borderColor: position.trailing_stop_activated ? '#10b98155' : position.trailingArmedByPrice ? '#f59e0b55' : '#334155',
                          boxShadow: position.trailing_stop_activated ? '0 0 18px rgba(16,185,129,0.18)' : undefined,
                        }}
                      >
                        ${position.trailingDisplayPrice.toFixed(6)}
                      </Badge>
                    ) : (
                      <div className="flex h-6 items-center justify-center text-slate-300">-</div>
                    )}
                    {typeof position.stop_loss_price === 'number' ? (
                      <Badge
                        className="h-6 w-full justify-center px-2 text-[11px] font-medium"
                        style={{
                          backgroundColor: '#0f172acc',
                          color: '#e2e8f0',
                          borderColor: '#334155',
                        }}
                      >
                        ${position.stop_loss_price.toFixed(6)}
                      </Badge>
                    ) : (
                      <div className="flex h-6 items-center justify-center text-slate-300">-</div>
                    )}
                  </div>

                  <div>
                    <div className="min-w-[112px] rounded-md border border-slate-700/70 bg-slate-900/60 px-2 py-1.5">
                      <div className="flex items-center justify-between gap-2 text-[10px] font-medium">
                        <span
                          style={{
                            color: position.trailing_stop_activated ? '#34d399' : position.trailingArmedByPrice ? '#fbbf24' : '#94a3b8',
                          }}
                        >
                          {position.trailingState.label}
                        </span>
                        <span className="text-[9px] text-slate-500">
                          {position.trailing_stop_activated
                            ? 'Trail on'
                            : position.trailingArmedByPrice
                              ? 'Ready'
                              : `${((position.distanceToArmPct ?? 0) * 100).toFixed(2)}%`}
                        </span>
                      </div>
                      <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-slate-800">
                        <div
                          className={cn(
                            'h-full rounded-full transition-all duration-500',
                            position.trailing_stop_activated
                              ? 'bg-emerald-400 shadow-[0_0_12px_rgba(52,211,153,0.55)]'
                              : position.trailingArmedByPrice
                                ? 'bg-amber-300'
                                : 'bg-slate-500'
                          )}
                          style={{
                            width: `${
                              position.trailing_stop_activated
                                ? 100
                                : Math.max(6, Math.round((position.trailingProgressPct ?? 0) * 100))
                            }%`,
                          }}
                        />
                      </div>
                    </div>
                  </div>

                  <div className="flex flex-wrap items-center gap-2 xl:justify-start">
                    <Badge
                      className="h-5 min-w-[120px] justify-center px-2 text-[10px] font-medium opacity-90"
                      style={regimeBadgeStyle(position.consensusSide)}
                    >
                      <span className="mr-1.5 text-[11px]">{regimeIcon(position.consensusSide)}</span>
                      Consensus
                    </Badge>
                    <Badge
                      className="h-5 min-w-[104px] justify-center px-2 text-[10px] font-medium opacity-90"
                      style={regimeBadgeStyle(position.bybitRegime)}
                    >
                      <span className="mr-1.5 text-[11px]">{regimeIcon(position.bybitRegime)}</span>
                      Bybit
                    </Badge>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}
