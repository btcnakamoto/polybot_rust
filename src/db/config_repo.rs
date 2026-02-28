use std::collections::HashMap;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RuntimeConfigEntry {
    pub key: String,
    pub value: String,
}

/// Get all runtime config entries.
pub async fn get_all_config(pool: &PgPool) -> anyhow::Result<Vec<RuntimeConfigEntry>> {
    let rows = sqlx::query_as::<_, RuntimeConfigEntry>(
        "SELECT key, value FROM runtime_config ORDER BY key",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Batch upsert runtime config entries.
pub async fn upsert_config(pool: &PgPool, entries: &HashMap<String, String>) -> anyhow::Result<()> {
    for (key, value) in entries {
        sqlx::query(
            r#"
            INSERT INTO runtime_config (key, value, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()
            "#,
        )
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    }

    Ok(())
}
