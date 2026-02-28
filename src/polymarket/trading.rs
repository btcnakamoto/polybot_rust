use std::sync::Arc;

use polymarket_client_sdk::clob::types::response::{OpenOrderResponse, PostOrderResponse};
use polymarket_client_sdk::clob::types::Side as SdkSide;
use polymarket_client_sdk::types::U256;
use rust_decimal::Decimal;

use super::wallet::PolymarketWallet;

/// Simplified trading interface wrapping the Polymarket SDK client.
pub struct TradingClient {
    wallet: Arc<PolymarketWallet>,
}

impl TradingClient {
    pub fn new(wallet: Arc<PolymarketWallet>) -> Self {
        Self { wallet }
    }

    /// Access the inner wallet reference.
    pub fn wallet(&self) -> &Arc<PolymarketWallet> {
        &self.wallet
    }

    /// Place a limit order on the CLOB.
    ///
    /// * `token_id` — CTF token ID (decimal string, e.g. from asset_id).
    /// * `side` — `"BUY"` or `"SELL"`.
    /// * `size` — Number of shares.
    /// * `price` — Price per share (0..1).
    pub async fn place_limit_order(
        &self,
        token_id: &str,
        side: &str,
        size: Decimal,
        price: Decimal,
    ) -> anyhow::Result<PostOrderResponse> {
        let sdk_side = match side.to_uppercase().as_str() {
            "BUY" => SdkSide::Buy,
            _ => SdkSide::Sell,
        };

        let token_id_u256 = U256::from_str_radix(token_id, 10)
            .or_else(|_| {
                // Try hex if decimal parse fails
                token_id
                    .strip_prefix("0x")
                    .map(|hex| U256::from_str_radix(hex, 16))
                    .unwrap_or_else(|| U256::from_str_radix(token_id, 16))
            })?;

        let client = self.wallet.client();
        let signer = self.wallet.signer();

        let signable_order = client
            .limit_order()
            .token_id(token_id_u256)
            .side(sdk_side)
            .price(price)
            .size(size)
            .build()
            .await?;

        let signed_order = client.sign(signer, signable_order).await?;
        let response = client.post_order(signed_order).await?;

        tracing::info!(
            order_id = ?response.order_id,
            status = ?response.status,
            "Order submitted to CLOB"
        );

        Ok(response)
    }

    /// Cancel a single order by CLOB order ID.
    pub async fn cancel_order(&self, order_id: &str) -> anyhow::Result<()> {
        self.wallet.client().cancel_order(order_id).await?;
        tracing::info!(order_id, "Order cancelled");
        Ok(())
    }

    /// Cancel all open orders.
    pub async fn cancel_all_orders(&self) -> anyhow::Result<()> {
        self.wallet.client().cancel_all_orders().await?;
        tracing::info!("All open orders cancelled");
        Ok(())
    }

    /// Query all open orders (first page).
    pub async fn get_open_orders(&self) -> anyhow::Result<Vec<OpenOrderResponse>> {
        let page = self
            .wallet
            .client()
            .orders(&Default::default(), None)
            .await?;
        Ok(page.data)
    }
}
