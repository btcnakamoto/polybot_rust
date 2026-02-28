export interface Whale {
  id: string;
  address: string;
  label?: string;
  category?: string;
  classification?: string;
  sharpe_ratio?: string;
  win_rate?: string;
  total_trades?: number;
  total_pnl?: string;
  kelly_fraction?: string;
  expected_value?: string;
  is_active?: boolean;
  last_trade_at?: string;
  created_at?: string;
  updated_at?: string;
}

export interface WhaleTrade {
  id: string;
  whale_id?: string;
  market_id: string;
  token_id: string;
  side: string;
  size: string;
  price: string;
  notional: string;
  tx_hash?: string;
  traded_at: string;
}

export interface CopyOrder {
  id: string;
  whale_trade_id?: string;
  market_id: string;
  token_id: string;
  side: string;
  size: string;
  target_price: string;
  fill_price?: string;
  slippage?: string;
  status: string;
  strategy: string;
  error_message?: string;
  placed_at?: string;
  filled_at?: string;
}

export interface Position {
  id: string;
  market_id: string;
  token_id: string;
  outcome: string;
  size: string;
  avg_entry_price: string;
  current_price?: string;
  unrealized_pnl?: string;
  realized_pnl?: string;
  status?: string;
  opened_at?: string;
  closed_at?: string;
}

export interface DashboardSummary {
  tracked_whales: number;
  active_positions: number;
  total_pnl: string;
  today_pnl: string;
  open_positions: number;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}
