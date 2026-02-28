-- Active markets table for market discovery
CREATE TABLE IF NOT EXISTS active_markets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    condition_id VARCHAR(200) NOT NULL UNIQUE,
    question TEXT NOT NULL,
    volume DECIMAL(18,2),
    liquidity DECIMAL(18,2),
    end_date_iso TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_active_markets_condition ON active_markets(condition_id);

-- Position SL/TP fields for exit strategy
ALTER TABLE positions ADD COLUMN IF NOT EXISTS stop_loss_pct NUMERIC(5,2) DEFAULT 15.0;
ALTER TABLE positions ADD COLUMN IF NOT EXISTS take_profit_pct NUMERIC(5,2) DEFAULT 50.0;
ALTER TABLE positions ADD COLUMN IF NOT EXISTS last_price_update TIMESTAMPTZ;
ALTER TABLE positions ADD COLUMN IF NOT EXISTS exit_reason TEXT;
ALTER TABLE positions ADD COLUMN IF NOT EXISTS exited_at TIMESTAMPTZ;
