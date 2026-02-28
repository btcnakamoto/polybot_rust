CREATE TABLE whales (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(42) NOT NULL UNIQUE,
    label VARCHAR(100),
    category VARCHAR(50),
    classification VARCHAR(20),
    sharpe_ratio DECIMAL(10,4),
    win_rate DECIMAL(5,4),
    total_trades INT DEFAULT 0,
    total_pnl DECIMAL(18,6),
    kelly_fraction DECIMAL(5,4),
    expected_value DECIMAL(18,6),
    is_active BOOLEAN DEFAULT true,
    last_trade_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_whales_active ON whales(is_active) WHERE is_active = true;
CREATE INDEX idx_whales_address ON whales(address);
CREATE INDEX idx_whales_category ON whales(category);
