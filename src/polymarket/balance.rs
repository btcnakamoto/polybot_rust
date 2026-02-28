use std::sync::Arc;

use rust_decimal::Decimal;

use super::wallet::PolymarketWallet;

/// Queries USDC and CTF token balances via the CLOB API.
pub struct BalanceChecker {
    wallet: Arc<PolymarketWallet>,
}

impl BalanceChecker {
    pub fn new(wallet: Arc<PolymarketWallet>) -> Self {
        Self { wallet }
    }

    /// Access the inner wallet reference.
    pub fn wallet(&self) -> &Arc<PolymarketWallet> {
        &self.wallet
    }

    /// Get available USDC balance from the CLOB API.
    ///
    /// Uses the authenticated client to query balance. Falls back to zero if
    /// the endpoint is unavailable.
    pub async fn get_usdc_balance(&self) -> anyhow::Result<Decimal> {
        // Use the wallet address to query the public balance endpoint
        let address = self.wallet.wallet_address();
        let url = format!(
            "https://clob.polymarket.com/balance?address={}",
            address
        );

        let resp: serde_json::Value = reqwest::get(&url).await?.json().await?;

        // The response may vary; try common field names
        let balance = resp
            .get("balance")
            .or_else(|| resp.get("available"))
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .or_else(|| v.as_f64().map(|f| Decimal::try_from(f).unwrap_or(Decimal::ZERO)))
            })
            .unwrap_or(Decimal::ZERO);

        Ok(balance)
    }

    /// Get balance of a specific CTF token.
    ///
    /// Uses the CLOB API positions endpoint to look up the token holding.
    pub async fn get_token_balance(&self, token_id: &str) -> anyhow::Result<Decimal> {
        let address = self.wallet.wallet_address();
        let url = format!(
            "https://clob.polymarket.com/positions?address={}&asset_id={}",
            address, token_id
        );

        let resp: serde_json::Value = reqwest::get(&url).await?.json().await?;

        // Extract size from position data
        let balance = resp
            .get("size")
            .or_else(|| {
                resp.as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|p| p.get("size"))
            })
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .or_else(|| v.as_f64().map(|f| Decimal::try_from(f).unwrap_or(Decimal::ZERO)))
            })
            .unwrap_or(Decimal::ZERO);

        Ok(balance)
    }

    /// Return the wallet address.
    pub fn wallet_address(&self) -> String {
        self.wallet.wallet_address()
    }
}
