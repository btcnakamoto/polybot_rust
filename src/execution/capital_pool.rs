use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Tracks available capital with reservation semantics.
///
/// When an order is placed, capital is *reserved* so that concurrent signals
/// cannot double-spend the same USDC.  On fill the reservation is confirmed
/// (capital is now in a position); on failure/cancel it is released.
#[derive(Clone)]
pub struct CapitalPool {
    inner: Arc<Mutex<CapitalInner>>,
}

struct CapitalInner {
    /// Total USDC balance (external source of truth).
    total_balance: Decimal,
    /// Capital reserved for in-flight orders (order_id → amount).
    reservations: HashMap<Uuid, Decimal>,
}

impl CapitalPool {
    /// Create a new pool seeded with an initial balance.
    pub fn new(initial_balance: Decimal) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CapitalInner {
                total_balance: initial_balance,
                reservations: HashMap::new(),
            })),
        }
    }

    /// Available capital = total_balance − sum(reservations).
    pub async fn available(&self) -> Decimal {
        let inner = self.inner.lock().await;
        let reserved: Decimal = inner.reservations.values().copied().sum();
        (inner.total_balance - reserved).max(Decimal::ZERO)
    }

    /// Reserve capital for a pending order.  Returns `false` if insufficient.
    pub async fn reserve(&self, order_id: Uuid, amount: Decimal) -> bool {
        let mut inner = self.inner.lock().await;
        let reserved: Decimal = inner.reservations.values().copied().sum();
        let available = (inner.total_balance - reserved).max(Decimal::ZERO);

        if amount > available {
            tracing::warn!(
                order_id = %order_id,
                required = %amount,
                available = %available,
                "Capital pool: insufficient funds to reserve"
            );
            return false;
        }

        inner.reservations.insert(order_id, amount);
        tracing::debug!(
            order_id = %order_id,
            amount = %amount,
            remaining = %(available - amount),
            "Capital pool: reserved"
        );
        true
    }

    /// Release a reservation (order failed / cancelled).
    pub async fn release(&self, order_id: &Uuid) {
        let mut inner = self.inner.lock().await;
        if let Some(amount) = inner.reservations.remove(order_id) {
            tracing::debug!(
                order_id = %order_id,
                amount = %amount,
                "Capital pool: released reservation"
            );
        }
    }

    /// Confirm a reservation (order filled — capital is now in a position).
    pub async fn confirm(&self, order_id: &Uuid) {
        let mut inner = self.inner.lock().await;
        if let Some(amount) = inner.reservations.remove(order_id) {
            // Reduce total balance since the capital is now locked in a position
            inner.total_balance -= amount;
            tracing::debug!(
                order_id = %order_id,
                amount = %amount,
                new_balance = %inner.total_balance,
                "Capital pool: confirmed fill, balance reduced"
            );
        }
    }

    /// Re-calibrate from the actual on-chain USDC balance.
    /// The new total is set to `external_balance`, reservations are kept.
    pub async fn sync_balance(&self, external_balance: Decimal) {
        let mut inner = self.inner.lock().await;
        let old = inner.total_balance;
        inner.total_balance = external_balance;
        tracing::info!(
            old_balance = %old,
            new_balance = %external_balance,
            active_reservations = inner.reservations.len(),
            "Capital pool: synced with external balance"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reserve_and_release() {
        let pool = CapitalPool::new(Decimal::from(1000));
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert!(pool.reserve(id1, Decimal::from(600)).await);
        assert_eq!(pool.available().await, Decimal::from(400));

        // Cannot reserve more than available
        assert!(!pool.reserve(id2, Decimal::from(500)).await);

        // Release first reservation
        pool.release(&id1).await;
        assert_eq!(pool.available().await, Decimal::from(1000));
    }

    #[tokio::test]
    async fn test_confirm_reduces_balance() {
        let pool = CapitalPool::new(Decimal::from(1000));
        let id = Uuid::new_v4();

        assert!(pool.reserve(id, Decimal::from(300)).await);
        pool.confirm(&id).await;

        // Balance reduced by confirmed amount, no reservation left
        assert_eq!(pool.available().await, Decimal::from(700));
    }

    #[tokio::test]
    async fn test_sync_balance() {
        let pool = CapitalPool::new(Decimal::from(1000));
        pool.sync_balance(Decimal::from(1500)).await;
        assert_eq!(pool.available().await, Decimal::from(1500));
    }
}
