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

    // Telegram notifications
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub notifications_enabled: bool,

    // Basket consensus
    pub basket_consensus_threshold: Decimal,
    pub basket_time_window_hours: i32,
    pub basket_min_wallets: i32,
    pub basket_max_wallets: i32,
    pub basket_enabled: bool,
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

            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN").ok(),
            telegram_chat_id: env::var("TELEGRAM_CHAT_ID").ok(),
            notifications_enabled: env::var("NOTIFICATIONS_ENABLED")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .unwrap_or(false),

            basket_consensus_threshold: env::var("BASKET_CONSENSUS_THRESHOLD")
                .unwrap_or_else(|_| "0.80".into())
                .parse()
                .unwrap_or(Decimal::new(80, 2)),
            basket_time_window_hours: env::var("BASKET_TIME_WINDOW_HOURS")
                .unwrap_or_else(|_| "48".into())
                .parse()
                .unwrap_or(48),
            basket_min_wallets: env::var("BASKET_MIN_WALLETS")
                .unwrap_or_else(|_| "5".into())
                .parse()
                .unwrap_or(5),
            basket_max_wallets: env::var("BASKET_MAX_WALLETS")
                .unwrap_or_else(|_| "10".into())
                .parse()
                .unwrap_or(10),
            basket_enabled: env::var("BASKET_ENABLED")
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

    /// Returns true if Telegram bot credentials are configured.
    pub fn has_telegram(&self) -> bool {
        self.telegram_bot_token.is_some() && self.telegram_chat_id.is_some()
    }
}
