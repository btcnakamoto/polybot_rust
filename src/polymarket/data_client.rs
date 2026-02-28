use reqwest::Client;
use thiserror::Error;

use super::types::{ApiMarket, ApiTrade};

const DATA_API_BASE: &str = "https://data-api.polymarket.com";

#[derive(Debug, Error)]
pub enum DataClientError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("unexpected response: {0}")]
    Unexpected(String),
}

#[derive(Debug, Clone)]
pub struct DataClient {
    http: Client,
    base_url: String,
}

impl DataClient {
    pub fn new(http: Client) -> Self {
        Self {
            http,
            base_url: DATA_API_BASE.into(),
        }
    }

    /// Fetch trades for a specific wallet address.
    pub async fn get_trades_by_wallet(
        &self,
        wallet: &str,
    ) -> Result<Vec<ApiTrade>, DataClientError> {
        let url = format!("{}/trades", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[("maker_address", wallet)])
            .send()
            .await?
            .error_for_status()?;

        let trades: Vec<ApiTrade> = resp.json().await?;
        Ok(trades)
    }

    /// Fetch a single market by condition ID.
    pub async fn get_market(&self, condition_id: &str) -> Result<ApiMarket, DataClientError> {
        let url = format!("{}/markets/{}", self.base_url, condition_id);
        let resp = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()?;

        let market: ApiMarket = resp.json().await?;
        Ok(market)
    }

    /// Fetch all active markets.
    pub async fn get_markets(&self) -> Result<Vec<ApiMarket>, DataClientError> {
        let url = format!("{}/markets", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()?;

        let markets: Vec<ApiMarket> = resp.json().await?;
        Ok(markets)
    }
}
