// Trading Types for ViperTrade Dashboard

export type TradingMode = 'paper' | 'testnet' | 'mainnet';
export type TradingProfile = 'CONSERVATIVE' | 'MEDIUM' | 'AGGRESSIVE';
export type PositionSide = 'Long' | 'Short';
export type TradeStatus = 'open' | 'closed' | 'liquidated' | 'cancelled' | 'rejected';
export type CloseReason = 'take_profit' | 'stop_loss' | 'trailing_stop' | 'time_exit' | 'manual' | 'liquidation' | 'circuit_breaker' | 'error';
export type EventSeverity = 'debug' | 'info' | 'warning' | 'error' | 'critical';
export type EventCategory = 'trade' | 'risk' | 'system' | 'notification' | 'reconciliation' | 'tupa' | 'circuit_breaker';

export interface Position {
  trade_id: string;
  symbol: string;
  side: PositionSide;
  quantity: number;
  notional_usdt: number;
  entry_price: number;
  leverage: number;
  trailing_stop_activated?: boolean;
  trailing_stop_peak_price?: number | null;
  trailing_stop_final_distance_pct?: number | null;
  stop_loss_price?: number | null;
  trailing_activation_price?: number | null;
  fixed_take_profit_price?: number | null;
  break_even_price?: number | null;
  opened_at: string;
}

export interface Trade {
  trade_id: string;
  symbol: string;
  side: PositionSide;
  status: TradeStatus;
  quantity: number;
  entry_price: number;
  exit_price?: number | null;
  pnl?: number | null;
  pnl_pct?: number | null;
  close_reason?: CloseReason | null;
  opened_at: string;
  closed_at?: string | null;
  duration_seconds?: number;
}

export interface SystemEvent {
  event_id: string;
  event_type: string;
  severity: EventSeverity;
  category?: EventCategory | null;
  symbol?: string | null;
  data?: Record<string, unknown>;
  timestamp: string;
}

export interface ServiceHealth {
  name: string;
  ok: boolean;
  status: number;
  latency_ms: number;
  url: string;
  error?: string;
}

export interface WalletBalance {
  ok: boolean;
  status?: number;
  url?: string;
  error?: string | null;
  ret_code?: number | null;
  ret_msg?: string | null;
  checked_at?: string;
  account_type?: string;
  total_equity?: number | null;
  wallet_balance?: number | null;
  margin_balance?: number | null;
  available_balance?: number | null;
  unrealized_pnl?: number | null;
  initial_margin?: number | null;
  maintenance_margin?: number | null;
  account_im_rate?: number | null;
  account_mm_rate?: number | null;
}

export interface PerformanceMetrics {
  last_24h?: {
    total_trades?: number;
    total_pnl?: number;
    win_rate?: number;
  };
  last_7d?: {
    total_trades?: number;
    total_pnl?: number;
    win_rate?: number;
  };
  last_30d?: {
    total_trades?: number;
    total_pnl?: number;
    win_rate?: number;
  };
  max_drawdown_30d?: number | null;
}

export interface RiskLimits {
  max_daily_loss_pct: number;
  max_leverage: number;
  risk_per_trade_pct: number;
}

export interface KillSwitchState {
  enabled: boolean;
  reason?: string | null;
  actor?: string | null;
  updated_at?: string | null;
}

export interface ExecutorState {
  enabled: boolean;
  reason?: string | null;
  actor?: string | null;
  updated_at?: string | null;
}

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
}

export interface DashboardPayload {
  source?: { baseUrl: string; fetchedAt: string };
  status?: {
    service?: string;
    risk_status?: string;
    trading_mode?: string;
    trading_profile?: string;
    trade_profile_label?: string;
    db_connected?: boolean;
    operator_controls_enabled?: boolean;
    critical_reconciliation_events_15m?: number;
    kill_switch?: KillSwitchState;
    executor?: ExecutorState;
    risk_limits?: RiskLimits;
  };
  performance?: PerformanceMetrics;
  positions?: { items: Position[] };
  trades?: { items: Trade[] };
  daily_trades_summary?: {
    ok?: boolean;
    count?: number;
    checked_at?: string;
  };
  events?: { items: SystemEvent[] };
  market_signals?: {
    updated_at?: string | null;
    items?: Record<string, MarketSignal> | MarketSignal[];
  };
  analytics_scores?: {
    updated_at?: string;
    horizon_minutes?: number;
    lookback_hours?: number;
  };
  wallet?: WalletBalance;
  risk_kpis?: {
    rejected_orders_24h?: number;
    open_exposure_usdt?: number;
    realized_pnl_24h?: number;
    critical_events_24h?: number;
  };
  control_state?: {
    operator_auth_mode?: string;
    operator_controls_enabled?: boolean;
    kill_switch?: KillSwitchState;
    executor?: ExecutorState;
    risk_limits?: RiskLimits;
  };
  services: ServiceHealth[];
  partial?: boolean;
  warnings?: string[];
}

export interface DashboardState {
  trading_mode: TradingMode;
  trading_profile: TradingProfile;
  executor_enabled: boolean;
  kill_switch_enabled: boolean;
  risk_limits: RiskLimits;
  wallet?: WalletBalance;
  positions: Position[];
  trades: Trade[];
  events: SystemEvent[];
  services: ServiceHealth[];
  performance?: PerformanceMetrics;
  market_signals?: MarketSignal[];
}
