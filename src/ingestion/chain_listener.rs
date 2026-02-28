use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::db::whale_repo;
use crate::models::{Side, WhaleTradeEvent};

/// CTF Exchange contract on Polygon.
const CTF_EXCHANGE: &str = "0x4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e";

/// NegRisk CTF Exchange contract on Polygon.
const NEG_RISK_CTF_EXCHANGE: &str = "0xc5d563a36ae78145c45a50134d48a1215220f80a";

/// Keccak256 of OrderFilled(bytes32,address,address,uint256,uint256,uint256,uint256,uint256)
const ORDER_FILLED_TOPIC: &str =
    "0xd0a08e8c493f9c94f29311604c9de1b4e8c8d4c06bd0c789af57f2d65bfec0f6";

const BASE_RECONNECT_DELAY: Duration = Duration::from_secs(2);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);
const WHALE_REFRESH_INTERVAL: Duration = Duration::from_secs(300);

/// USDC on Polygon has 6 decimals.
const USDC_DECIMALS: u32 = 6;

/// Run the Polygon chain listener, subscribing to OrderFilled events on
/// CTF Exchange contracts and forwarding matching whale trades into the pipeline.
pub async fn run_chain_listener(
    ws_url: String,
    pool: PgPool,
    trade_tx: mpsc::Sender<WhaleTradeEvent>,
) {
    let mut attempt: u32 = 0;

    // Load initial whale address set
    let mut whale_addresses = load_whale_addresses(&pool).await;
    tracing::info!(
        whale_count = whale_addresses.len(),
        "Chain listener loaded whale addresses"
    );
    let mut last_refresh = tokio::time::Instant::now();

    loop {
        tracing::info!(url = %ws_url, "Chain listener connecting to Polygon WSS...");

        match connect_async(&ws_url).await {
            Ok((ws_stream, _response)) => {
                tracing::info!("Chain listener connected to Polygon WSS");
                attempt = 0;

                let (mut write, mut read) = ws_stream.split();

                // Send eth_subscribe for logs
                let subscribe_msg = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "eth_subscribe",
                    "params": ["logs", {
                        "address": [CTF_EXCHANGE, NEG_RISK_CTF_EXCHANGE],
                        "topics": [[ORDER_FILLED_TOPIC]]
                    }]
                });

                if let Err(e) = write
                    .send(Message::Text(subscribe_msg.to_string().into()))
                    .await
                {
                    tracing::error!(error = %e, "Failed to send eth_subscribe");
                    continue;
                }
                tracing::info!("Subscribed to OrderFilled events on 2 contracts");

                loop {
                    // Periodically refresh whale addresses
                    if last_refresh.elapsed() >= WHALE_REFRESH_INTERVAL {
                        whale_addresses = load_whale_addresses(&pool).await;
                        last_refresh = tokio::time::Instant::now();
                        tracing::debug!(
                            whale_count = whale_addresses.len(),
                            "Refreshed whale address set"
                        );
                    }

                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    handle_rpc_message(
                                        text.as_ref(),
                                        &whale_addresses,
                                        &trade_tx,
                                    ).await;
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    if let Err(e) = write.send(Message::Pong(data)).await {
                                        tracing::warn!(error = %e, "Failed to send pong");
                                        break;
                                    }
                                }
                                Some(Ok(Message::Close(_))) => {
                                    tracing::warn!("Chain listener: server sent close frame");
                                    break;
                                }
                                Some(Ok(_)) => {}
                                Some(Err(e)) => {
                                    tracing::error!(error = %e, "Chain listener: WS read error");
                                    break;
                                }
                                None => {
                                    tracing::warn!("Chain listener: WS stream ended");
                                    break;
                                }
                            }
                        }
                        _ = sleep(WHALE_REFRESH_INTERVAL) => {
                            // Triggers the refresh check at the top of the loop
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Chain listener: connection failed");
            }
        }

        // Exponential backoff
        let delay = BASE_RECONNECT_DELAY * 2u32.saturating_pow(attempt);
        let delay = delay.min(MAX_RECONNECT_DELAY);
        attempt = attempt.saturating_add(1);
        tracing::info!(delay_secs = delay.as_secs(), attempt, "Chain listener reconnecting...");
        sleep(delay).await;
    }
}

/// Load active whale addresses from DB as a lowercase HashSet.
async fn load_whale_addresses(pool: &PgPool) -> HashSet<String> {
    match whale_repo::get_active_whales(pool).await {
        Ok(whales) => whales
            .into_iter()
            .map(|w| w.address.to_lowercase())
            .collect(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to load whale addresses");
            HashSet::new()
        }
    }
}

/// Handle an incoming JSON-RPC message from the Polygon WSS node.
async fn handle_rpc_message(
    text: &str,
    whale_addresses: &HashSet<String>,
    trade_tx: &mpsc::Sender<WhaleTradeEvent>,
) {
    let msg: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Subscription confirmations: {"jsonrpc":"2.0","id":1,"result":"0x..."}
    if msg.get("id").is_some() && msg.get("result").is_some() {
        tracing::debug!(
            result = %msg["result"],
            "Chain listener: subscription confirmed"
        );
        return;
    }

    // Subscription notifications: {"jsonrpc":"2.0","method":"eth_subscription","params":{...}}
    let params = match msg.get("params") {
        Some(p) => p,
        None => return,
    };
    let result = match params.get("result") {
        Some(r) => r,
        None => return,
    };

    // Parse the log entry
    let topics = match result.get("topics").and_then(|t| t.as_array()) {
        Some(t) => t,
        None => return,
    };

    if topics.len() < 4 {
        return;
    }

    // Verify event signature
    let event_sig = topics[0].as_str().unwrap_or_default();
    if event_sig != ORDER_FILLED_TOPIC {
        return;
    }

    // topics[1] = orderHash (ignored)
    // topics[2] = maker address (bytes32 padded)
    // topics[3] = taker address (bytes32 padded)
    let maker = extract_address(topics[2].as_str().unwrap_or_default());
    let taker = extract_address(topics[3].as_str().unwrap_or_default());

    let maker_is_whale = whale_addresses.contains(&maker);
    let taker_is_whale = whale_addresses.contains(&taker);

    if !maker_is_whale && !taker_is_whale {
        return;
    }

    // Parse data: 5 x uint256 (makerAssetId, takerAssetId, makerAmountFilled, takerAmountFilled, fee)
    let data_hex = result
        .get("data")
        .and_then(|d| d.as_str())
        .unwrap_or_default();
    let data_hex = data_hex.strip_prefix("0x").unwrap_or(data_hex);

    if data_hex.len() < 320 {
        // 5 * 64 hex chars
        tracing::warn!(
            data_len = data_hex.len(),
            "Chain event: data too short for OrderFilled"
        );
        return;
    }

    let maker_asset_id = &data_hex[0..64];
    let taker_asset_id = &data_hex[64..128];
    let maker_amount_filled = parse_uint256_decimal(&data_hex[128..192], USDC_DECIMALS);
    let taker_amount_filled = parse_uint256_decimal(&data_hex[192..256], USDC_DECIMALS);
    // fee = data_hex[256..320] — not used

    // Determine whale wallet, side, asset_id, size, price
    let (wallet, side, asset_id, size, price) = if maker_is_whale {
        determine_trade_params(
            &maker,
            true, // is_maker
            maker_asset_id,
            taker_asset_id,
            maker_amount_filled,
            taker_amount_filled,
        )
    } else {
        determine_trade_params(
            &taker,
            false, // is_taker
            maker_asset_id,
            taker_asset_id,
            maker_amount_filled,
            taker_amount_filled,
        )
    };

    let notional = size * price;

    let event = WhaleTradeEvent {
        wallet,
        market_id: asset_id.clone(),
        asset_id,
        side,
        size,
        price,
        notional,
        timestamp: Utc::now(),
    };

    tracing::info!(
        wallet = %event.wallet,
        side = %event.side,
        size = %event.size,
        price = %event.price,
        notional = %event.notional,
        "Chain event: whale trade detected"
    );

    if let Err(e) = trade_tx.send(event).await {
        tracing::error!(error = %e, "Failed to send chain trade event to pipeline");
    }
}

/// Extract a 20-byte address from a 32-byte zero-padded hex topic.
/// Input: "0x000000000000000000000000abcdef1234567890abcdef1234567890abcdef12"
/// Output: "0xabcdef1234567890abcdef1234567890abcdef12"
fn extract_address(topic: &str) -> String {
    let hex = topic.strip_prefix("0x").unwrap_or(topic);
    if hex.len() < 40 {
        return format!("0x{hex}");
    }
    // Take last 40 hex chars (20 bytes)
    let addr = &hex[hex.len() - 40..];
    format!("0x{addr}").to_lowercase()
}

/// Parse a 64-char hex uint256 into a Decimal with the given decimal places.
fn parse_uint256_decimal(hex: &str, decimals: u32) -> Decimal {
    // Use u128 which handles up to ~3.4e38 — sufficient for USDC amounts
    let value = u128::from_str_radix(hex.trim_start_matches('0'), 16).unwrap_or(0);
    let mut d = Decimal::from(value);
    let divisor = Decimal::from(10u64.pow(decimals));
    d /= divisor;
    d
}

/// Determine trade parameters based on whether the whale is maker or taker.
///
/// In the CTF Exchange:
/// - A BUY of outcome tokens: the buyer gives USDC (fungible), receives outcome tokens (ERC-1155)
/// - A SELL of outcome tokens: the seller gives outcome tokens, receives USDC
///
/// For the maker:
///   - makerAssetId is what the maker gives, takerAssetId is what the maker receives
///   - If makerAssetId is 0 (USDC placeholder), maker is buying outcome tokens
///   - If makerAssetId != 0, maker is selling outcome tokens
///
/// For the taker:
///   - takerAssetId is what the taker gives, makerAssetId is what the taker receives
///   - If takerAssetId is 0 (USDC placeholder), taker is buying outcome tokens
///   - If takerAssetId != 0, taker is selling outcome tokens
fn determine_trade_params(
    whale_addr: &str,
    is_maker: bool,
    maker_asset_id_hex: &str,
    taker_asset_id_hex: &str,
    maker_amount: Decimal,
    taker_amount: Decimal,
) -> (String, Side, String, Decimal, Decimal) {
    let maker_asset_is_zero = is_zero_asset(maker_asset_id_hex);
    let taker_asset_is_zero = is_zero_asset(taker_asset_id_hex);

    if is_maker {
        if maker_asset_is_zero {
            // Maker gives USDC → buying outcome tokens (takerAssetId)
            let asset_id = format_asset_id(taker_asset_id_hex);
            let price = safe_divide(maker_amount, taker_amount);
            (whale_addr.to_string(), Side::Buy, asset_id, taker_amount, price)
        } else {
            // Maker gives outcome tokens → selling
            let asset_id = format_asset_id(maker_asset_id_hex);
            let price = safe_divide(taker_amount, maker_amount);
            (whale_addr.to_string(), Side::Sell, asset_id, maker_amount, price)
        }
    } else {
        // Taker
        if taker_asset_is_zero {
            // Taker gives USDC → buying outcome tokens (makerAssetId)
            let asset_id = format_asset_id(maker_asset_id_hex);
            let price = safe_divide(taker_amount, maker_amount);
            (whale_addr.to_string(), Side::Buy, asset_id, maker_amount, price)
        } else {
            // Taker gives outcome tokens → selling
            let asset_id = format_asset_id(taker_asset_id_hex);
            let price = safe_divide(maker_amount, taker_amount);
            (whale_addr.to_string(), Side::Sell, asset_id, taker_amount, price)
        }
    }
}

/// Check if a hex-encoded uint256 asset ID is zero (represents USDC side of the trade).
fn is_zero_asset(hex: &str) -> bool {
    hex.trim_start_matches('0').is_empty()
}

/// Convert a 64-char hex uint256 to its full decimal string representation.
/// ERC-1155 token IDs are 256-bit values that overflow u128, so we use
/// manual hex-to-decimal conversion via digit-by-digit arithmetic.
fn format_asset_id(hex: &str) -> String {
    let hex = hex.trim_start_matches('0');
    if hex.is_empty() {
        return "0".to_string();
    }

    // Try u128 first (fast path for small values)
    if hex.len() <= 32 {
        if let Ok(v) = u128::from_str_radix(hex, 16) {
            return v.to_string();
        }
    }

    // Full uint256: accumulate decimal digits from hex
    let mut digits: Vec<u8> = vec![0];
    for ch in hex.chars() {
        let hex_digit = match ch {
            '0'..='9' => ch as u8 - b'0',
            'a'..='f' => ch as u8 - b'a' + 10,
            'A'..='F' => ch as u8 - b'A' + 10,
            _ => return hex.to_string(),
        };

        // Multiply current decimal number by 16
        let mut carry: u16 = 0;
        for d in digits.iter_mut().rev() {
            let val = *d as u16 * 16 + carry;
            *d = (val % 10) as u8;
            carry = val / 10;
        }
        while carry > 0 {
            digits.insert(0, (carry % 10) as u8);
            carry /= 10;
        }

        // Add the hex digit
        carry = hex_digit as u16;
        for d in digits.iter_mut().rev() {
            let val = *d as u16 + carry;
            *d = (val % 10) as u8;
            carry = val / 10;
        }
        while carry > 0 {
            digits.insert(0, (carry % 10) as u8);
            carry /= 10;
        }
    }

    digits.iter().map(|d| (d + b'0') as char).collect()
}

/// Safe division that returns ZERO on divide-by-zero.
fn safe_divide(numerator: Decimal, denominator: Decimal) -> Decimal {
    if denominator.is_zero() {
        Decimal::ZERO
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_address() {
        let topic = "0x0000000000000000000000004bfb41d5b3570defd03c39a9a4d8de6bd8b8982e";
        assert_eq!(
            extract_address(topic),
            "0x4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e"
        );
    }

    #[test]
    fn test_extract_address_short() {
        assert_eq!(extract_address("0xabcd"), "0xabcd");
    }

    #[test]
    fn test_parse_uint256_decimal() {
        // 1_000_000 in hex = 0xF4240, padded to 64 chars
        let hex = "00000000000000000000000000000000000000000000000000000000000f4240";
        let result = parse_uint256_decimal(hex, 6);
        assert_eq!(result, Decimal::from(1)); // 1_000_000 / 10^6 = 1.0
    }

    #[test]
    fn test_parse_uint256_decimal_large() {
        // 50_000_000 (50 USDC) = 0x2FAF080
        let hex = "0000000000000000000000000000000000000000000000000000000002faf080";
        let result = parse_uint256_decimal(hex, 6);
        assert_eq!(result, Decimal::from(50));
    }

    #[test]
    fn test_is_zero_asset() {
        let zero = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(is_zero_asset(zero));

        let nonzero = "0000000000000000000000000000000000000000000000000000000002faf080";
        assert!(!is_zero_asset(nonzero));
    }

    #[test]
    fn test_format_asset_id_small() {
        let hex = "0000000000000000000000000000000000000000000000000000000002faf080";
        assert_eq!(format_asset_id(hex), "50000000");
    }

    #[test]
    fn test_format_asset_id_uint256() {
        // Real Polymarket CLOB token ID that overflows u128
        let hex = "7581b394f5a4dd19ec46e4ff36baa3a841c9eeb80af0f0850be552c0fece2d87";
        let result = format_asset_id(hex);
        assert_eq!(
            result,
            "53149765984136093709083310870325314268796238675098813080656099381431327665543"
        );
        // Verify it's a large decimal string (not hex fallback)
        assert!(result.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_format_asset_id_zero() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(format_asset_id(hex), "0");
    }

    #[test]
    fn test_safe_divide() {
        assert_eq!(safe_divide(Decimal::from(10), Decimal::from(2)), Decimal::from(5));
        assert_eq!(safe_divide(Decimal::from(10), Decimal::ZERO), Decimal::ZERO);
    }

    #[test]
    fn test_determine_trade_params_maker_buy() {
        // Maker gives USDC (asset 0), receives outcome tokens
        let zero_asset = "0000000000000000000000000000000000000000000000000000000000000000";
        let token_asset = "0000000000000000000000000000000000000000000000000000000000000064"; // 100

        let (wallet, side, asset_id, size, price) = determine_trade_params(
            "0xwhale",
            true,
            zero_asset,
            token_asset,
            Decimal::from(50),  // maker gives 50 USDC
            Decimal::from(100), // taker gives 100 tokens
        );

        assert_eq!(wallet, "0xwhale");
        assert_eq!(side, Side::Buy);
        assert_eq!(asset_id, "100");
        assert_eq!(size, Decimal::from(100));
        // price = 50/100 = 0.5
        assert_eq!(price, Decimal::new(5, 1));
    }

    #[test]
    fn test_determine_trade_params_taker_sell() {
        // Taker gives outcome tokens, receives USDC
        let zero_asset = "0000000000000000000000000000000000000000000000000000000000000000";
        let token_asset = "0000000000000000000000000000000000000000000000000000000000000064"; // 100

        let (wallet, side, asset_id, size, price) = determine_trade_params(
            "0xwhale",
            false,
            zero_asset,   // maker asset: USDC (what taker receives)
            token_asset,  // taker asset: tokens (what taker gives)
            Decimal::from(30),  // maker gives 30 USDC
            Decimal::from(100), // taker gives 100 tokens
        );

        assert_eq!(wallet, "0xwhale");
        assert_eq!(side, Side::Sell);
        assert_eq!(asset_id, "100");
        assert_eq!(size, Decimal::from(100));
        // price = 30/100 = 0.3
        assert_eq!(price, Decimal::new(3, 1));
    }
}
