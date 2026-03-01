-- Add peak_price column for trailing stop tracking
ALTER TABLE positions ADD COLUMN IF NOT EXISTS peak_price DECIMAL(10,6);

-- Backfill: set peak_price = GREATEST(avg_entry_price, current_price) for open positions
UPDATE positions SET peak_price = GREATEST(avg_entry_price, COALESCE(current_price, avg_entry_price))
WHERE status = 'open' AND peak_price IS NULL;

-- Lower take_profit_pct from 50% to 20% for existing open positions
UPDATE positions SET take_profit_pct = 20.0 WHERE status = 'open' AND take_profit_pct = 50.0;
