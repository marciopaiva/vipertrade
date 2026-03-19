/**
 * Trading Types
 * 
 * TypeScript type definitions for trading data structures.
 */

export interface MarketSignal {
  symbol: string;
  current_price: number;
  bybit_price?: number;
  atr_14: number;
  volume_24h: number;
  funding_rate: number;
  trend_score: number;
  spread_pct: number;
  regime?: string;
  consensus_side?: string;
  consensus_count?: number;
  exchanges_available?: number;
  bybit_regime?: string;
}

export interface Position {
  trade_id: string;
  symbol: string;
  side: 'Long' | 'Short';
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

export interface Trade {
  trade_id: string;
  symbol: string;
  side: 'Long' | 'Short';
  status: 'open' | 'closed' | 'liquidated' | 'cancelled' | 'rejected';
  quantity: number;
  entry_price: number;
  exit_price?: number | null;
  pnl?: number | null;
  pnl_pct?: number | null;
  close_reason?: string | null;
  opened_at: string;
  closed_at?: string | null;
  duration_seconds?: number;
}

export interface Wallet {
  account_type: string;
  total_equity: number;
  wallet_balance: number;
  margin_balance: number;
  available_balance: number;
  unrealized_pnl: number;
  initial_margin: number;
  maintenance_margin: number;
  account_im_rate: number;
  account_mm_rate: number;
}

export interface ServiceHealth {
  name: string;
  ok: boolean;
  status: number;
  latency_ms: number;
  url: string;
  error?: string;
}

export interface DashboardData {
  status: {
    trading_mode: string;
    trading_profile: string;
    risk_status: string;
    db_connected: boolean;
    executor: {
      enabled: boolean;
      reason?: string | null;
      actor?: string | null;
      updated_at?: string | null;
    };
    kill_switch: {
      enabled: boolean;
      reason?: string | null;
      actor?: string | null;
      updated_at?: string | null;
    };
    risk_limits: {
      max_daily_loss_pct: number;
      max_leverage: number;
      risk_per_trade_pct: number;
    };
  };
  performance: {
    last_24h?: {
      total_trades: number;
      total_pnl: number;
      win_rate: number;
    };
    last_7d?: {
      total_trades: number;
      total_pnl: number;
      win_rate: number;
    };
    max_drawdown_30d?: number | null;
  };
  positions: {
    items: Position[];
  };
  trades: {
    items: Trade[];
  };
  wallet?: Wallet;
  services: ServiceHealth[];
  events: Array<{
    event_id: string;
    event_type: string;
    severity: string;
    category?: string | null;
    symbol?: string | null;
    timestamp: string;
  }>;
  market_signals?: {
    updated_at?: string | null;
    items?: Record<string, MarketSignal> | MarketSignal[];
  };
}

export type TradingMode = 'paper' | 'testnet' | 'mainnet';
export type TradingProfile = 'CONSERVATIVE' | 'MEDIUM' | 'AGGRESSIVE';
export type PositionSide = 'Long' | 'Short';
export type TradeStatus = 'open' | 'closed' | 'liquidated' | 'cancelled' | 'rejected';
