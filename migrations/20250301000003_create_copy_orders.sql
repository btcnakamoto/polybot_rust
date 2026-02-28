CREATE TABLE copy_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    whale_trade_id UUID REFERENCES whale_trades(id),
    market_id VARCHAR(100) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    side VARCHAR(4) NOT NULL,
    size DECIMAL(18,6) NOT NULL,
    target_price DECIMAL(10,6) NOT NULL,
    fill_price DECIMAL(10,6),
    slippage DECIMAL(10,6),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    strategy VARCHAR(20) NOT NULL,
    error_message TEXT,
    placed_at TIMESTAMPTZ DEFAULT NOW(),
    filled_at TIMESTAMPTZ
);

CREATE INDEX idx_copy_orders_status ON copy_orders(status) WHERE status IN ('pending', 'partial');
CREATE INDEX idx_copy_orders_whale_trade ON copy_orders(whale_trade_id);
