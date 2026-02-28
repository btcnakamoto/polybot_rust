use rust_decimal::Decimal;
use serde_json::json;

use crate::models::{CopyOrder, WhaleTradeEvent};

/// Telegram notification service. Failures are logged but never block the main flow.
#[derive(Debug, Clone)]
pub struct Notifier {
    http: reqwest::Client,
    bot_token: String,
    chat_id: String,
}

impl Notifier {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            bot_token,
            chat_id,
        }
    }

    /// Send a Telegram message. Failures are logged as warnings.
    pub async fn send(&self, message: &str) {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        let body = json!({
            "chat_id": self.chat_id,
            "text": message,
            "parse_mode": "Markdown",
        });

        match self.http.post(&url).json(&body).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!(
                        status = %resp.status(),
                        "Telegram sendMessage returned non-2xx"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to send Telegram notification");
            }
        }
    }
}

/// Format a whale trade alert message.
pub fn format_whale_alert(event: &WhaleTradeEvent) -> String {
    let wallet_short = if event.wallet.len() > 10 {
        format!("{}...{}", &event.wallet[..6], &event.wallet[event.wallet.len()-4..])
    } else {
        event.wallet.clone()
    };

    format!(
        "*Whale Alert*\nWallet: `{}`\nSide: {}\nSize: {} @ {}\nNotional: {} USDC\nMarket: `{}`",
        wallet_short,
        event.side,
        event.size,
        event.price,
        event.notional,
        &event.market_id[..16.min(event.market_id.len())],
    )
}

/// Format an order result message.
pub fn format_order_result(order: &CopyOrder, success: bool, error: Option<&str>) -> String {
    if success {
        format!(
            "*Order Filled*\nSide: {}\nSize: {} @ {}\nMarket: `{}`",
            order.side,
            order.size,
            order.fill_price.unwrap_or(order.target_price),
            &order.market_id[..16.min(order.market_id.len())],
        )
    } else {
        format!(
            "*Order Failed*\nSide: {}\nSize: {}\nMarket: `{}`\nError: {}",
            order.side,
            order.size,
            &order.market_id[..16.min(order.market_id.len())],
            error.unwrap_or("unknown"),
        )
    }
}

/// Format a consensus alert message.
pub fn format_consensus_alert(basket_name: &str, direction: &str, consensus_pct: Decimal) -> String {
    format!(
        "*Basket Consensus*\nBasket: {}\nDirection: {}\nConsensus: {}%",
        basket_name,
        direction,
        consensus_pct * Decimal::from(100),
    )
}
