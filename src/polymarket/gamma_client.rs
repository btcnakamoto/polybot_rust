use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";

#[derive(Debug, Error)]
pub enum GammaClientError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("unexpected response: {0}")]
    Unexpected(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaToken {
    pub token_id: String,
    pub outcome: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaMarket {
    pub condition_id: String,
    pub question: String,
    #[serde(default)]
    pub tokens: Vec<GammaToken>,
    #[serde(default)]
    pub volume: Option<String>,
    #[serde(default)]
    pub liquidity: Option<String>,
    #[serde(default)]
    pub end_date_iso: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GammaClient {
    http: Client,
    base_url: String,
}

impl Default for GammaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GammaClient {
    pub fn new() -> Self {
        Self {
            http: Client::new(),
            base_url: GAMMA_API_BASE.into(),
        }
    }

    /// Fetch active markets from the Gamma API with pagination.
    pub async fn get_active_markets(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<GammaMarket>, GammaClientError> {
        let url = format!("{}/markets", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("active", "true"),
                ("closed", "false"),
                ("limit", &limit.to_string()),
                ("offset", &offset.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;

        let markets: Vec<GammaMarket> = resp.json().await?;
        Ok(markets)
    }
}
