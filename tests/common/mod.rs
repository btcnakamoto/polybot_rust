use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use polybot::models::{Whale, WhaleTrade};

/// Connect to the test database and run all migrations.
#[allow(dead_code)]
pub async fn setup_test_db() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://polybot:password@localhost:5432/polybot_test".into());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Clean tables for test isolation
    sqlx::query("DELETE FROM consensus_signals").execute(&pool).await.ok();
    sqlx::query("DELETE FROM basket_wallets").execute(&pool).await.ok();
    sqlx::query("DELETE FROM whale_baskets").execute(&pool).await.ok();
    sqlx::query("DELETE FROM positions").execute(&pool).await.ok();
    sqlx::query("DELETE FROM copy_orders").execute(&pool).await.ok();
    sqlx::query("DELETE FROM whale_trades").execute(&pool).await.ok();
    sqlx::query("DELETE FROM market_outcomes").execute(&pool).await.ok();
    sqlx::query("DELETE FROM whales").execute(&pool).await.ok();

    pool
}

/// Seed a whale record for testing.
#[allow(dead_code)]
pub async fn seed_whale(
    pool: &PgPool,
    address: &str,
    win_rate: Decimal,
    classification: &str,
) -> Whale {
    sqlx::query_as::<_, Whale>(
        r#"
        INSERT INTO whales (address, win_rate, classification, is_active)
        VALUES ($1, $2, $3, true)
        ON CONFLICT (address) DO UPDATE
            SET win_rate = $2, classification = $3, is_active = true, updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(address)
    .bind(win_rate)
    .bind(classification)
    .fetch_one(pool)
    .await
    .expect("Failed to seed whale")
}

/// Seed a trade record for testing.
#[allow(dead_code)]
pub async fn seed_trade(
    pool: &PgPool,
    whale_id: uuid::Uuid,
    market_id: &str,
    side: &str,
    notional: Decimal,
    days_ago: i64,
) -> WhaleTrade {
    let traded_at = Utc::now() - Duration::days(days_ago);

    sqlx::query_as::<_, WhaleTrade>(
        r#"
        INSERT INTO whale_trades (whale_id, market_id, token_id, side, size, price, notional, traded_at)
        VALUES ($1, $2, 'token_test', $3, $4, 0.65, $4, $5)
        RETURNING *
        "#,
    )
    .bind(whale_id)
    .bind(market_id)
    .bind(side)
    .bind(notional)
    .bind(traded_at)
    .fetch_one(pool)
    .await
    .expect("Failed to seed trade")
}
