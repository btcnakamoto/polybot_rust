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

    // Wallet & execution
    pub private_key: Option<String>,
    pub polygon_rpc_url: String,
    pub dry_run: bool,

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

    // Market discovery
    pub market_discovery_enabled: bool,
    pub market_discovery_interval_secs: u64,
    pub market_min_volume: Decimal,
    pub market_min_liquidity: Decimal,

    // Whale seeder
    pub whale_seeder_enabled: bool,
    pub whale_seeder_skip_top_n: usize,
    pub whale_seeder_min_trades: u32,

    // Whale trade poller
    pub whale_poller_interval_secs: u64,

    // Chain listener (Polygon on-chain OrderFilled events)
    pub chain_listener_enabled: bool,
    pub polygon_ws_url: Option<String>,

    // Exit strategy (SL/TP)
    pub default_stop_loss_pct: Decimal,
    pub default_take_profit_pct: Decimal,
    pub position_monitor_interval_secs: u64,

    // Pipeline signal quality
    pub tracked_whale_min_notional: Decimal,
    pub min_resolved_for_signal: i32,
    pub min_signal_win_rate: Decimal,
    pub min_total_trades_for_signal: i32,
    pub min_signal_notional: Decimal,
    pub max_signal_notional: Decimal,
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

            private_key: env::var("PRIVATE_KEY").ok(),
            polygon_rpc_url: env::var("RPC_URL")
                .unwrap_or_else(|_| "https://polygon-rpc.com".into()),
            dry_run: env::var("DRY_RUN")
                .unwrap_or_else(|_| "true".into())
                .parse()
                .unwrap_or(true),

            copy_strategy: env::var("COPY_STRATEGY").unwrap_or_else(|_| "kelly".into()),
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

            market_discovery_enabled: env::var("MARKET_DISCOVERY_ENABLED")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .unwrap_or(false),
            market_discovery_interval_secs: env::var("MARKET_DISCOVERY_INTERVAL")
                .unwrap_or_else(|_| "300".into())
                .parse()
                .unwrap_or(300),
            market_min_volume: env::var("MARKET_MIN_VOLUME")
                .unwrap_or_else(|_| "10000".into())
                .parse()
                .unwrap_or(Decimal::from(10_000)),
            market_min_liquidity: env::var("MARKET_MIN_LIQUIDITY")
                .unwrap_or_else(|_| "5000".into())
                .parse()
                .unwrap_or(Decimal::from(5_000)),

            whale_seeder_enabled: env::var("WHALE_SEEDER_ENABLED")
                .unwrap_or_else(|_| "true".into())
                .parse()
                .unwrap_or(true),
            whale_seeder_skip_top_n: env::var("WHALE_SEEDER_SKIP_TOP_N")
                .unwrap_or_else(|_| "10".into())
                .parse()
                .unwrap_or(10),
            whale_seeder_min_trades: env::var("WHALE_SEEDER_MIN_TRADES")
                .unwrap_or_else(|_| "100".into())
                .parse()
                .unwrap_or(100),

            whale_poller_interval_secs: env::var("WHALE_POLLER_INTERVAL")
                .unwrap_or_else(|_| "60".into())
                .parse()
                .unwrap_or(60),

            chain_listener_enabled: env::var("CHAIN_LISTENER_ENABLED")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .unwrap_or(false),
            polygon_ws_url: env::var("POLYGON_WS_URL").ok(),

            default_stop_loss_pct: env::var("STOP_LOSS_PCT")
                .unwrap_or_else(|_| "15.0".into())
                .parse()
                .unwrap_or(Decimal::new(1500, 2)),
            default_take_profit_pct: env::var("TAKE_PROFIT_PCT")
                .unwrap_or_else(|_| "50.0".into())
                .parse()
                .unwrap_or(Decimal::new(5000, 2)),
            position_monitor_interval_secs: env::var("POSITION_MONITOR_INTERVAL")
                .unwrap_or_else(|_| "30".into())
                .parse()
                .unwrap_or(30),

            tracked_whale_min_notional: env::var("TRACKED_WHALE_MIN_NOTIONAL")
                .unwrap_or_else(|_| "500".into())
                .parse()
                .unwrap_or(Decimal::from(500)),
            min_resolved_for_signal: env::var("MIN_RESOLVED_FOR_SIGNAL")
                .unwrap_or_else(|_| "5".into())
                .parse()
                .unwrap_or(5),
            min_signal_win_rate: env::var("MIN_SIGNAL_WIN_RATE")
                .unwrap_or_else(|_| "0.60".into())
                .parse()
                .unwrap_or(Decimal::new(60, 2)),
            min_total_trades_for_signal: env::var("MIN_TOTAL_TRADES_FOR_SIGNAL")
                .unwrap_or_else(|_| "50".into())
                .parse()
                .unwrap_or(50),
            min_signal_notional: env::var("MIN_SIGNAL_NOTIONAL")
                .unwrap_or_else(|_| "50000".into())
                .parse()
                .unwrap_or(Decimal::from(50_000)),
            max_signal_notional: env::var("MAX_SIGNAL_NOTIONAL")
                .unwrap_or_else(|_| "500000".into())
                .parse()
                .unwrap_or(Decimal::from(500_000)),
        })
    }

    /// Returns true if a private key is configured for on-chain signing.
    pub fn has_private_key(&self) -> bool {
        self.private_key.is_some()
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
