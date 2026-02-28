pub mod basket_repo;
pub mod order_repo;
pub mod position_repo;
pub mod trade_repo;
pub mod whale_repo;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn init_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    // Verify connectivity
    sqlx::query("SELECT 1").execute(&pool).await?;

    Ok(pool)
}
