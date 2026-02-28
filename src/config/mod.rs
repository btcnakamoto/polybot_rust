use rust_decimal::Decimal;
use std::env;

const DEFAULT_WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub redis_url: Option<String>,

    // Polymarket API credentials (optional — required for authenticated endpoints)
    pub polymarket_api_key: Option<String>,
    pub polymarket_api_secret: Option<String>,
    pub polymarket_passphrase: Option<String>,

    // WebSocket
    pub polymarket_ws_url: String,
    pub ws_subscribe_token_ids: Vec<String>,

    // Execution
    pub copy_strategy: String,
    pub bankroll: Decimal,
    pub base_copy_amount: Decimal,
    pub copy_enabled: bool,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let token_ids_raw = env::var("WS_SUBSCRIBE_TOKEN_IDS").unwrap_or_default();
        let ws_subscribe_token_ids: Vec<String> = token_ids_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?,
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()?,
            redis_url: env::var("REDIS_URL").ok(),

            polymarket_api_key: env::var("POLYMARKET_API_KEY").ok(),
            polymarket_api_secret: env::var("POLYMARKET_API_SECRET").ok(),
            polymarket_passphrase: env::var("POLYMARKET_PASSPHRASE").ok(),

            polymarket_ws_url: env::var("POLYMARKET_WS_URL")
                .unwrap_or_else(|_| DEFAULT_WS_URL.into()),
            ws_subscribe_token_ids,

            copy_strategy: env::var("COPY_STRATEGY").unwrap_or_else(|_| "fixed".into()),
            bankroll: env::var("BANKROLL")
                .unwrap_or_else(|_| "1000".into())
                .parse()
                .unwrap_or(Decimal::from(1_000)),
            base_copy_amount: env::var("BASE_COPY_AMOUNT")
                .unwrap_or_else(|_| "50".into())
                .parse()
                .unwrap_or(Decimal::from(50)),
            copy_enabled: env::var("COPY_ENABLED")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .unwrap_or(false),
        })
    }

    /// Returns true if all Polymarket API credentials are configured.
    pub fn has_polymarket_auth(&self) -> bool {
        self.polymarket_api_key.is_some()
            && self.polymarket_api_secret.is_some()
            && self.polymarket_passphrase.is_some()
    }
}
