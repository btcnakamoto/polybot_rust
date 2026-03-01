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
pub struct GammaEvent {
    #[serde(default)]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaMarket {
    #[serde(alias = "conditionId")]
    pub condition_id: String,
    pub question: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub events: Vec<GammaEvent>,
    /// JSON array of outcome labels, e.g. ["Yes","No"] or ["G2 Esports","Karmine Corp"]
    #[serde(default)]
    pub outcomes: Option<String>,
    /// Stringified JSON array of token IDs, e.g. "[\"token1\", \"token2\"]"
    #[serde(default, alias = "clobTokenIds")]
    pub clob_token_ids: Option<String>,
    #[serde(default)]
    pub volume: Option<String>,
    #[serde(default)]
    pub liquidity: Option<String>,
    #[serde(default, alias = "endDateIso")]
    pub end_date_iso: Option<String>,
}

impl GammaMarket {
    /// Parse the stringified clobTokenIds into a Vec of token ID strings.
    pub fn parse_token_ids(&self) -> Vec<String> {
        self.clob_token_ids
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    /// Get the event-level slug (for polymarket.com/event/{slug} URLs).
    /// Falls back to the market-level slug if no event slug is available.
    pub fn event_slug(&self) -> Option<&str> {
        self.events
            .first()
            .and_then(|e| e.slug.as_deref())
            .or(self.slug.as_deref())
    }

    /// Serialize the outcomes field for storage.
    /// The Gamma API returns outcomes as a JSON array string like `["Yes","No"]`.
    pub fn outcomes_str(&self) -> Option<&str> {
        self.outcomes.as_deref()
    }
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
