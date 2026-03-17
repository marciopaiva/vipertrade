'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

interface MarketSignal {
  symbol: string;
  current_price: number;
  bybit_price?: number;
}

interface Position {
  trade_id: string;
  symbol: string;
  side: string;
  quantity: number;
  notional_usdt: number;
  entry_price: number;
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
      ? { label: 'Trailing Active', tone: 'active' as const }
      : trailingArmedByPrice
        ? { label: 'Ready to Arm', tone: 'armed' as const }
        : { label: 'Waiting', tone: 'waiting' as const };

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
    };
  });

  return (
    <Card className={cn('bg-gradient-to-br from-slate-900/90 via-slate-800/80 to-slate-900/90 border-slate-700/50', className)}>
      <CardHeader className="pb-2">
        <CardTitle className="text-lg text-slate-200">Open Positions ({positions.length})</CardTitle>
      </CardHeader>
      <CardContent className="pt-0">
        <div className="space-y-2">
          {enrichedPositions.map((position) => {
            const pnlColor = (position.unrealizedPnl ?? 0) >= 0 ? '#10b981' : '#ef4444';
            const sideColor = position.side.toLowerCase() === 'long' ? '#10b981' : '#ef4444';

            return (
              <div
                key={position.trade_id}
                className="bg-slate-800/50 rounded-lg border border-slate-700/50 p-3"
              >
                <div className="flex items-center justify-between gap-4">
                  {/* Left: Symbol + Side + Qty */}
                  <div className="flex items-center gap-2 min-w-[120px]">
                    <div>
                      <div className="text-sm font-bold text-slate-200">{position.symbol}</div>
                      <div className="text-xs text-slate-500">Qty: {position.quantity.toLocaleString()}</div>
                    </div>
                    <Badge
                      style={{ backgroundColor: sideColor + '22', color: sideColor, borderColor: sideColor + '55' }}
                      className="text-xs px-1.5 py-0.5 h-5"
                    >
                      {position.side.toUpperCase()}
                    </Badge>
                  </div>

                  {/* Center: PnL */}
                  <div className="text-right min-w-[100px]">
                    <div className="text-sm font-bold" style={{ color: pnlColor }}>
                      {position.unrealizedPnl !== null ? `$${position.unrealizedPnl.toFixed(2)}` : '-'}
                    </div>
                    <div className="text-xs" style={{ color: pnlColor }}>
                      {position.unrealizedPnlPct !== null ? `${(position.unrealizedPnlPct! * 100).toFixed(2)}%` : '-'}
                    </div>
                  </div>

                  {/* Right: Entry/Mark/Delta/Notional */}
                  <div className="hidden md:flex items-center gap-4 text-xs min-w-[280px]">
                    <div>
                      <div className="text-slate-500">Entry</div>
                      <div className="text-slate-300">${position.entryPrice.toFixed(6)}</div>
                    </div>
                    <div>
                      <div className="text-slate-500">Mark</div>
                      <div className="text-slate-300">
                        {position.markPrice ? `$${position.markPrice.toFixed(6)}` : '-'}
                      </div>
                    </div>
                    <div>
                      <div className="text-slate-500">Delta</div>
                      <div className="font-semibold" style={{ color: position.markDeltaPct !== null && position.markDeltaPct! >= 0 ? '#10b981' : '#ef4444' }}>
                        {position.markDeltaPct !== null ? `${(position.markDeltaPct! * 100).toFixed(2)}%` : '-'}
                      </div>
                    </div>
                    <div>
                      <div className="text-slate-500">Notional</div>
                      <div className="text-slate-300">${position.notional_usdt.toLocaleString()}</div>
                    </div>
                  </div>

                  {/* Far Right: Trailing Status */}
                  <div className="hidden lg:flex items-center gap-3 min-w-[200px]">
                    <div className="flex-1">
                      <div className="text-xs text-slate-300 truncate">{position.triggerState}</div>
                      <div className="flex items-center gap-2 mt-1">
                        <Badge
                          style={{
                            backgroundColor: position.trailing_stop_activated ? '#10b98122' : position.trailingArmedByPrice ? '#f59e0b22' : '#64748b22',
                            color: position.trailing_stop_activated ? '#10b981' : position.trailingArmedByPrice ? '#f59e0b' : '#64748b',
                            borderColor: position.trailing_stop_activated ? '#10b98155' : position.trailingArmedByPrice ? '#f59e0b55' : '#64748b55'
                          }}
                          className="text-xs px-1.5 py-0.5 h-5"
                        >
                          {position.trailingState.label}
                        </Badge>
                        {position.trailingProgressPct !== null && (
                          <div className="flex items-center gap-1">
                            <div className="w-16 h-1 bg-slate-700 rounded-full overflow-hidden">
                              <div
                                className="h-full bg-gradient-to-r from-cyan-500 to-green-500"
                                style={{ width: `${Math.max(8, Math.round(position.trailingProgressPct! * 100))}%` }}
                              />
                            </div>
                            <div className="text-xs text-slate-400 w-6 text-right">
                              {Math.round(position.trailingProgressPct! * 100)}%
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
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
