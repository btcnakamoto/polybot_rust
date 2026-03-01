-- Add CLOB order ID tracking to copy_orders for fill confirmation
ALTER TABLE copy_orders ADD COLUMN clob_order_id VARCHAR(200);

CREATE INDEX idx_copy_orders_clob_order_id ON copy_orders(clob_order_id) WHERE clob_order_id IS NOT NULL;

-- Add 'submitted' to the status index for fill poller lookups
DROP INDEX IF EXISTS idx_copy_orders_status;
CREATE INDEX idx_copy_orders_status ON copy_orders(status) WHERE status IN ('pending', 'submitted', 'partial');
