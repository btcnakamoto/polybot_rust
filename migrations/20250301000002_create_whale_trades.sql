CREATE TABLE whale_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    whale_id UUID REFERENCES whales(id),
    market_id VARCHAR(100) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    side VARCHAR(4) NOT NULL,
    size DECIMAL(18,6) NOT NULL,
    price DECIMAL(10,6) NOT NULL,
    notional DECIMAL(18,6) NOT NULL,
    tx_hash VARCHAR(66),
    traded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_whale_trades_whale ON whale_trades(whale_id, traded_at DESC);
CREATE INDEX idx_whale_trades_market ON whale_trades(market_id);
