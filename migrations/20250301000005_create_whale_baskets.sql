-- Phase 5: Whale Baskets — consensus-driven copy signals
-- Tables: whale_baskets, basket_wallets, consensus_signals

-- Basket definitions
CREATE TABLE IF NOT EXISTS whale_baskets (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        VARCHAR(100) NOT NULL UNIQUE,
    category    VARCHAR(20) NOT NULL DEFAULT 'crypto',  -- politics / crypto / sports
    consensus_threshold DECIMAL(5,4) NOT NULL DEFAULT 0.8000,
    time_window_hours   INT NOT NULL DEFAULT 48,
    min_wallets INT NOT NULL DEFAULT 5,
    max_wallets INT NOT NULL DEFAULT 10,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_whale_baskets_category ON whale_baskets (category);
CREATE INDEX idx_whale_baskets_active ON whale_baskets (is_active) WHERE is_active = true;

-- Many-to-many: basket ↔ whale
CREATE TABLE IF NOT EXISTS basket_wallets (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    basket_id   UUID NOT NULL REFERENCES whale_baskets(id) ON DELETE CASCADE,
    whale_id    UUID NOT NULL REFERENCES whales(id) ON DELETE CASCADE,
    added_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (basket_id, whale_id)
);

CREATE INDEX idx_basket_wallets_basket ON basket_wallets (basket_id);
CREATE INDEX idx_basket_wallets_whale ON basket_wallets (whale_id);

-- Consensus signal audit log
CREATE TABLE IF NOT EXISTS consensus_signals (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    basket_id           UUID NOT NULL REFERENCES whale_baskets(id) ON DELETE CASCADE,
    market_id           VARCHAR(256) NOT NULL,
    direction           VARCHAR(4) NOT NULL,          -- BUY / SELL
    consensus_pct       DECIMAL(5,4) NOT NULL,
    participating_whales INT NOT NULL,
    total_whales        INT NOT NULL,
    triggered_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_consensus_signals_basket ON consensus_signals (basket_id);
CREATE INDEX idx_consensus_signals_market ON consensus_signals (market_id);
CREATE INDEX idx_consensus_signals_time ON consensus_signals (triggered_at DESC);
