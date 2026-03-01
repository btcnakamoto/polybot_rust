-- Store clob_token_ids in active_markets for token_id → question lookups.
-- The chain listener produces decimal token_ids; this column lets us map
-- them back to the market question for human-readable notifications.
ALTER TABLE active_markets ADD COLUMN clob_token_ids TEXT;
