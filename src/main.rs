use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use rust_decimal::Decimal;
use tokio::sync::broadcast;

use polybot::api::router::create_router;
use polybot::api::ws_types::WsMessage;
use polybot::config::AppConfig;
use polybot::execution::capital_pool::CapitalPool;
use polybot::execution::copy_engine::{self, CopyEngineConfig};
use polybot::execution::order_executor::OrderExecutor;
use polybot::execution::position_sizer::SizingStrategy;
use polybot::execution::risk_manager::RiskLimits;
use polybot::ingestion::chain_listener::run_chain_listener;
use polybot::ingestion::pipeline::{apply_runtime_overrides, process_trade_event, PipelineConfig};
use polybot::ingestion::ws_listener::run_ws_listener;
use polybot::models::{CopySignal, WhaleTradeEvent};
use std::collections::HashMap;
use polybot::polymarket::{
    BalanceChecker, ClobClient, DataClient, GammaClient, PolymarketAuth, PolymarketWallet,
    TradingClient,
};
use polybot::services::notifier::Notifier;
use polybot::{db, metrics, services, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install rustls crypto provider before any TLS usage
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let addr = format!("{}:{}", config.host, config.port);

    // --- Prometheus metrics ---
    let metrics_handle = metrics::init_metrics();
    tracing::info!("Prometheus metrics initialized");

    tracing::info!("Connecting to database...");
    let db = db::init_pool(&config.database_url).await?;
    tracing::info!("Database connected");

    // Run pending migrations
    sqlx::migrate!("./migrations")
        .run(&db)
        .await?;
    tracing::info!("Database migrations applied");

    // --- Telegram notifier ---
    let notifier: Option<Arc<Notifier>> = if config.notifications_enabled && config.has_telegram() {
        let n = Notifier::new(
            config.telegram_bot_token.clone().unwrap(),
            config.telegram_chat_id.clone().unwrap(),
        );
        tracing::info!("Telegram notifier enabled");
        Some(Arc::new(n))
    } else {
        tracing::info!("Telegram notifications disabled");
        None
    };

    // --- Global pause flag ---
    let pause_flag = Arc::new(AtomicBool::new(false));

    // --- Wallet & trading client initialization ---
    let wallet: Option<Arc<PolymarketWallet>>;
    let trading_client: Option<Arc<TradingClient>>;
    let balance_checker: Option<Arc<BalanceChecker>>;

    if config.has_private_key() {
        let pk = config.private_key.as_ref().unwrap();
        match PolymarketWallet::new(pk).await {
            Ok(w) => {
                let w = Arc::new(w);
                tracing::info!(
                    address = %w.wallet_address(),
                    "Wallet initialized"
                );

                let tc = TradingClient::new(Arc::clone(&w));
                let bc = BalanceChecker::new(Arc::clone(&w));

                // Query and log USDC balance at startup
                match bc.get_usdc_balance().await {
                    Ok(usdc) => tracing::info!(balance = %usdc, "USDC balance"),
                    Err(e) => tracing::warn!(error = %e, "Failed to query USDC balance at startup"),
                }

                trading_client = Some(Arc::new(tc));
                balance_checker = Some(Arc::new(bc));
                wallet = Some(w);
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize wallet — falling back to monitor-only mode");
                wallet = None;
                trading_client = None;
                balance_checker = None;
            }
        }
    } else {
        tracing::warn!("No private key — running in monitor-only mode");
        wallet = None;
        trading_client = None;
        balance_checker = None;
    };

    // --- Whale seeder (periodic: seed new whales + deactivate stale ones) ---
    if config.whale_seeder_enabled {
        let seeder_data_client = DataClient::new(reqwest::Client::new());
        let seeder_db = db.clone();
        let seeder_config = config.clone();
        let seeder_interval = 6 * 3600; // Re-check every 6 hours
        tokio::spawn(async move {
            services::whale_seeder::run_whale_seeder_loop(
                seeder_data_client,
                seeder_db,
                seeder_config,
                seeder_interval,
            )
            .await;
        });
        tracing::info!(interval_secs = 6 * 3600, "Whale seeder spawned (periodic)");
    } else {
        tracing::info!("Whale seeder disabled (WHALE_SEEDER_ENABLED=false)");
    }

    // --- Market resolution poller ---
    {
        let poller_db = db.clone();
        let data_client = DataClient::new(reqwest::Client::new());
        let notifier_clone = notifier.clone();
        tokio::spawn(async move {
            services::resolution::run_resolution_poller(poller_db, data_client, 300, notifier_clone).await;
        });
        tracing::info!("Market resolution poller spawned (interval=300s)");
    }

    // --- Execution layer: copy engine ---
    let (signal_tx, signal_rx) = tokio::sync::mpsc::channel::<CopySignal>(500);

    // --- Capital pool ---
    // In dry-run mode always use config.bankroll (no real USDC needed).
    // In live mode use actual USDC balance, falling back to bankroll if query fails.
    let dry_run_mode = config.dry_run || trading_client.is_none();
    let initial_balance = if dry_run_mode {
        config.bankroll
    } else if let Some(ref bc) = balance_checker {
        let bal = bc.get_usdc_balance().await.unwrap_or(config.bankroll);
        if bal == Decimal::ZERO { config.bankroll } else { bal }
    } else {
        config.bankroll
    };
    let capital_pool = CapitalPool::new(initial_balance);
    tracing::info!(initial_balance = %initial_balance, "Capital pool initialized");

    if config.copy_enabled {
        let clob_client = if config.has_polymarket_auth() {
            let auth = PolymarketAuth::new(
                config.polymarket_api_key.clone().unwrap(),
                config.polymarket_api_secret.clone().unwrap(),
                config.polymarket_passphrase.clone().unwrap(),
            );
            Some(ClobClient::new(reqwest::Client::new(), auth))
        } else {
            tracing::warn!("No Polymarket API credentials — orderbook slippage checks disabled");
            None
        };

        let dry_run = config.dry_run || trading_client.is_none();
        if dry_run {
            tracing::info!("Copy engine running in DRY-RUN mode");
        } else {
            tracing::info!("Copy engine running in LIVE mode");
        }

        let engine_config = CopyEngineConfig {
            strategy: SizingStrategy::parse_strategy(&config.copy_strategy),
            bankroll: config.bankroll,
            base_amount: config.base_copy_amount,
            risk_limits: RiskLimits::default(),
            dry_run,
            default_stop_loss_pct: config.default_stop_loss_pct,
            default_take_profit_pct: config.default_take_profit_pct,
        };

        // Build OrderExecutor with optional TradingClient for live execution
        let executor_trading = wallet.as_ref().map(|w| TradingClient::new(Arc::clone(w)));
        let executor = OrderExecutor::new(
            executor_trading,
            clob_client,
            RiskLimits::default(),
            dry_run,
        );

        let engine_db = db.clone();
        let engine_notifier = notifier.clone();
        let engine_balance = wallet.as_ref().map(|w| BalanceChecker::new(Arc::clone(w)));
        let engine_pause = Arc::clone(&pause_flag);
        let engine_capital = capital_pool.clone();

        tokio::spawn(async move {
            copy_engine::run_copy_engine(
                signal_rx,
                engine_db,
                executor,
                engine_config,
                engine_notifier,
                engine_balance,
                engine_pause,
                engine_capital,
            )
            .await;
        });

        tracing::info!(
            strategy = %config.copy_strategy,
            bankroll = %config.bankroll,
            "Copy engine spawned"
        );

        // --- Order fill poller (live mode only) ---
        if !dry_run {
            if let Some(ref tc) = trading_client {
                let poller_db = db.clone();
                let poller_tc = Arc::clone(tc);
                let poller_capital = capital_pool.clone();
                let poller_config = CopyEngineConfig {
                    strategy: SizingStrategy::parse_strategy(&config.copy_strategy),
                    bankroll: config.bankroll,
                    base_amount: config.base_copy_amount,
                    risk_limits: RiskLimits::default(),
                    dry_run: false,
                    default_stop_loss_pct: config.default_stop_loss_pct,
                    default_take_profit_pct: config.default_take_profit_pct,
                };

                tokio::spawn(async move {
                    services::order_fill_poller::run_order_fill_poller(
                        poller_db,
                        poller_tc,
                        poller_capital,
                        poller_config,
                        10, // poll every 10 seconds
                    )
                    .await;
                });
                tracing::info!("Order fill poller spawned (interval=10s)");
            }
        }

        // --- Balance sync task (every 60s, live mode only) ---
        if !dry_run {
            if let Some(ref bc_arc) = balance_checker {
                let sync_capital = capital_pool.clone();
                let sync_bc = BalanceChecker::new(Arc::clone(bc_arc.wallet()));
                tokio::spawn(async move {
                    let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(60));
                    loop {
                        ticker.tick().await;
                        match sync_bc.get_usdc_balance().await {
                            Ok(balance) => {
                                sync_capital.sync_balance(balance).await;
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Balance sync: failed to fetch USDC balance");
                            }
                        }
                    }
                });
                tracing::info!("Balance sync task spawned (interval=60s)");
            }
        }
    } else {
        tracing::info!("Copy engine disabled (COPY_ENABLED=false)");
        // Drop the receiver so pipeline doesn't block
        drop(signal_rx);
    }

    // --- Watch channel for dynamic token subscription ---
    let initial_tokens = config.ws_subscribe_token_ids.clone();
    let (token_tx, token_rx) = tokio::sync::watch::channel(initial_tokens.clone());

    // --- Market discovery ---
    if config.market_discovery_enabled {
        let gamma_client = GammaClient::new();
        let discovery_db = db.clone();
        let discovery_interval = config.market_discovery_interval_secs;
        let min_volume = config.market_min_volume;
        let min_liquidity = config.market_min_liquidity;

        tokio::spawn(async move {
            services::market_discovery::run_market_discovery(
                gamma_client,
                token_tx,
                discovery_db,
                discovery_interval,
                min_volume,
                min_liquidity,
            )
            .await;
        });
        tracing::info!(
            interval = config.market_discovery_interval_secs,
            "Market discovery spawned"
        );
    } else {
        tracing::info!("Market discovery disabled (MARKET_DISCOVERY_ENABLED=false)");
    }

    // --- Position monitor (SL/TP) ---
    if config.has_polymarket_auth() {
        let auth = PolymarketAuth::new(
            config.polymarket_api_key.clone().unwrap(),
            config.polymarket_api_secret.clone().unwrap(),
            config.polymarket_passphrase.clone().unwrap(),
        );
        let monitor_clob = ClobClient::new(reqwest::Client::new(), auth);
        let monitor_db = db.clone();
        let monitor_tc = trading_client.clone();
        let monitor_dry = config.dry_run || trading_client.is_none();
        let monitor_pause = Arc::clone(&pause_flag);
        let monitor_interval = config.position_monitor_interval_secs;
        let monitor_notifier = notifier.clone();

        tokio::spawn(async move {
            services::position_monitor::run_position_monitor(
                monitor_db,
                monitor_clob,
                monitor_tc,
                monitor_dry,
                monitor_pause,
                monitor_interval,
                monitor_notifier,
            )
            .await;
        });
        tracing::info!(
            interval = config.position_monitor_interval_secs,
            "Position monitor spawned (SL/TP)"
        );
    } else {
        tracing::info!("Position monitor disabled (no Polymarket auth credentials)");
    }

    // --- Data pipeline: ingestion → intelligence → execution ---
    let (trade_tx, mut trade_rx) = tokio::sync::mpsc::channel::<WhaleTradeEvent>(1000);

    // WebSocket listener for market price awareness
    if !initial_tokens.is_empty() || config.market_discovery_enabled {
        let ws_url = config.polymarket_ws_url.clone();
        let ws_trade_tx = trade_tx.clone();
        tracing::info!(
            initial_tokens = initial_tokens.len(),
            market_discovery = config.market_discovery_enabled,
            "Starting WebSocket listener"
        );
        tokio::spawn(async move {
            run_ws_listener(ws_url, token_rx, ws_trade_tx).await;
        });
    } else {
        tracing::warn!("No token IDs and market discovery disabled — WebSocket listener will not start");
    }

    // Chain listener — low-latency on-chain OrderFilled event monitoring
    let chain_listener_active = config.chain_listener_enabled && config.polygon_ws_url.is_some();
    if chain_listener_active {
        let chain_ws_url = config.polygon_ws_url.clone().unwrap();
        let chain_db = db.clone();
        let chain_tx = trade_tx.clone();
        tokio::spawn(async move {
            run_chain_listener(chain_ws_url, chain_db, chain_tx).await;
        });
        tracing::info!("Chain listener spawned (Polygon WSS OrderFilled events)");
    } else if config.chain_listener_enabled {
        tracing::warn!("Chain listener enabled but POLYGON_WS_URL not set — skipping");
    }

    // Whale trade poller — fallback/backup mechanism for detecting tracked whale trades
    // When chain listener is active, increase interval to 300s (catch-up only)
    {
        let poller_data_client = DataClient::new(reqwest::Client::new());
        let poller_db = db.clone();
        let poller_tx = trade_tx.clone();
        let poller_interval = if chain_listener_active {
            tracing::info!("Whale poller interval increased to 300s (chain listener active)");
            300
        } else {
            config.whale_poller_interval_secs
        };

        tokio::spawn(async move {
            services::whale_trade_poller::run_whale_trade_poller(
                poller_data_client,
                poller_db,
                poller_tx,
                poller_interval,
            )
            .await;
        });
        tracing::info!(
            interval = poller_interval,
            "Whale trade poller spawned"
        );
    }

    // Drop the original sender so the pipeline shuts down when all senders are done
    drop(trade_tx);

    // Pipeline consumer: intelligence + signal emission
    {
        let pipeline_db = db.clone();
        let copy_enabled = config.copy_enabled;
        let pipeline_notifier = notifier.clone();
        let pipeline_config = PipelineConfig {
            tracked_whale_min_notional: config.tracked_whale_min_notional,
            min_signal_win_rate: config.min_signal_win_rate,
            min_resolved_for_signal: config.min_resolved_for_signal,
            min_total_trades_for_signal: config.min_total_trades_for_signal,
            signal_notional_liquidity_pct: config.signal_notional_liquidity_pct,
            signal_notional_floor: config.signal_notional_floor,
            max_signal_notional: config.max_signal_notional,
            min_signal_ev: config.min_signal_ev,
            assumed_slippage_pct: config.assumed_slippage_pct,
            signal_dedup_window_secs: 10,
        };
        let dedup_state = Arc::new(tokio::sync::Mutex::new(HashMap::<String, Instant>::new()));
        tokio::spawn(async move {
            let signal_sender = if copy_enabled { Some(&signal_tx) } else { None };
            while let Some(event) = trade_rx.recv().await {
                tracing::debug!(
                    wallet = %event.wallet,
                    notional = %event.notional,
                    "WhaleTradeEvent received in pipeline"
                );
                let effective_config = apply_runtime_overrides(&pipeline_config, &pipeline_db).await;
                if let Err(e) = process_trade_event(
                    &event,
                    &pipeline_db,
                    signal_sender,
                    pipeline_notifier.as_deref(),
                    &effective_config,
                    &dedup_state,
                ).await {
                    tracing::error!(
                        error = %e,
                        wallet = %event.wallet,
                        "Pipeline processing failed"
                    );
                }
            }
            tracing::warn!("WhaleTradeEvent channel closed");
        });
    }

    // --- WebSocket broadcast channel for dashboard ---
    let (ws_broadcast_tx, _) = broadcast::channel::<WsMessage>(256);

    let state = AppState {
        db,
        config,
        ws_tx: ws_broadcast_tx,
        metrics_handle,
        notifier,
        wallet,
        trading_client,
        balance_checker,
        pause_flag,
    };
    let router = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {addr}");

    // --- Graceful shutdown ---
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Shutting down gracefully...");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
    tracing::info!("Received SIGINT (Ctrl+C), starting graceful shutdown...");
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer())
        .init();
}
