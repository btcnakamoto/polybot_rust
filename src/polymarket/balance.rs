use std::sync::Arc;

use polymarket_client_sdk::clob::types::request::BalanceAllowanceRequest;
use polymarket_client_sdk::clob::types::AssetType;
use polymarket_client_sdk::types::U256;
use rust_decimal::Decimal;

use super::wallet::PolymarketWallet;

/// Queries USDC and CTF token balances via the authenticated CLOB API.
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

    /// Get available USDC balance from the authenticated CLOB API.
    pub async fn get_usdc_balance(&self) -> anyhow::Result<Decimal> {
        let request = BalanceAllowanceRequest::builder()
            .asset_type(AssetType::Collateral)
            .build();

        let resp = self.wallet.client().balance_allowance(request).await?;
        Ok(resp.balance)
    }

    /// Get balance of a specific CTF token.
    pub async fn get_token_balance(&self, token_id: &str) -> anyhow::Result<Decimal> {
        let token_id_u256 = U256::from_str_radix(token_id, 10)
            .or_else(|_| {
                token_id
                    .strip_prefix("0x")
                    .map(|hex| U256::from_str_radix(hex, 16))
                    .unwrap_or_else(|| U256::from_str_radix(token_id, 16))
            })?;

        let request = BalanceAllowanceRequest::builder()
            .asset_type(AssetType::Conditional)
            .token_id(token_id_u256)
            .build();

        let resp = self.wallet.client().balance_allowance(request).await?;
        Ok(resp.balance)
    }

    /// Return the wallet address.
    pub fn wallet_address(&self) -> String {
        self.wallet.wallet_address()
    }
}
