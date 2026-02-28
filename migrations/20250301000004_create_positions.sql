CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id VARCHAR(100) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    outcome VARCHAR(10) NOT NULL,
    size DECIMAL(18,6) NOT NULL,
    avg_entry_price DECIMAL(10,6) NOT NULL,
    current_price DECIMAL(10,6),
    unrealized_pnl DECIMAL(18,6),
    status VARCHAR(10) DEFAULT 'open',
    opened_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    realized_pnl DECIMAL(18,6)
);

CREATE INDEX idx_positions_open ON positions(status) WHERE status = 'open';
CREATE INDEX idx_positions_market ON positions(market_id);
