use rust_decimal::Decimal;
use reqwest::Client;
use serde::{Deserialize, Serialize};
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

/// A single entry from the Polymarket leaderboard (/v1/leaderboard).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeaderboardEntry {
    #[serde(default, alias = "proxyWallet")]
    pub address: Option<String>,
    #[serde(default, alias = "vol")]
    pub volume: Option<Decimal>,
    #[serde(default)]
    pub pnl: Option<Decimal>,
    #[serde(default)]
    pub rank: Option<String>,
    #[serde(default, alias = "userName")]
    pub user_name: Option<String>,
}

/// A single trade from the user trades endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserTrade {
    #[serde(default, alias = "asset")]
    pub token_id: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub size: Option<Decimal>,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    pub timestamp: Option<serde_json::Value>,
    #[serde(default, alias = "conditionId")]
    pub market: Option<String>,
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

    /// Fetch leaderboard entries from the Polymarket data API.
    pub async fn get_leaderboard(
        &self,
        limit: u32,
    ) -> Result<Vec<LeaderboardEntry>, DataClientError> {
        let url = format!("{}/v1/leaderboard", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("limit", limit.to_string()),
                ("timePeriod", "ALL".into()),
                ("orderBy", "PNL".into()),
            ])
            .send()
            .await?
            .error_for_status()?;

        let entries: Vec<LeaderboardEntry> = resp.json().await?;
        Ok(entries)
    }

    /// Fetch recent trades for a specific user address.
    pub async fn get_user_trades(
        &self,
        address: &str,
        limit: u32,
    ) -> Result<Vec<UserTrade>, DataClientError> {
        let url = format!("{}/trades", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("user", address.to_string()),
                ("limit", limit.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;

        let trades: Vec<UserTrade> = resp.json().await?;
        Ok(trades)
    }
}
