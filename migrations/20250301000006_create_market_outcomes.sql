-- Market resolution tracking
CREATE TABLE IF NOT EXISTS market_outcomes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id VARCHAR(256) NOT NULL UNIQUE,
    token_id VARCHAR(256),
    outcome VARCHAR(20) NOT NULL DEFAULT 'unresolved',
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_market_outcomes_unresolved ON market_outcomes (outcome) WHERE outcome = 'unresolved';
CREATE INDEX idx_market_outcomes_market_id ON market_outcomes (market_id);
