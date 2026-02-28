use reqwest::{Client, RequestBuilder};
use thiserror::Error;

use super::auth::PolymarketAuth;
use super::types::{ApiMarket, ApiOrderBook};

const CLOB_API_BASE: &str = "https://clob.polymarket.com";

#[derive(Debug, Error)]
pub enum ClobClientError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("authentication error: {0}")]
    Auth(#[from] super::auth::AuthError),

    #[error("unexpected response: {0}")]
    Unexpected(String),
}

#[derive(Debug, Clone)]
pub struct ClobClient {
    http: Client,
    auth: PolymarketAuth,
    base_url: String,
}

impl ClobClient {
    pub fn new(http: Client, auth: PolymarketAuth) -> Self {
        Self {
            http,
            auth,
            base_url: CLOB_API_BASE.into(),
        }
    }

    /// Build an authenticated GET request with HMAC signature headers.
    fn authenticated_get(&self, path: &str) -> Result<RequestBuilder, ClobClientError> {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let signature = self.auth.sign(&timestamp, "GET", path, "")?;

        let url = format!("{}{}", self.base_url, path);
        let req = self
            .http
            .get(&url)
            .header("POLY-API-KEY", &self.auth.api_key)
            .header("POLY-SIGNATURE", signature)
            .header("POLY-TIMESTAMP", &timestamp)
            .header("POLY-PASSPHRASE", &self.auth.passphrase);

        Ok(req)
    }

    /// Fetch markets from the CLOB API (authenticated).
    pub async fn get_markets(&self) -> Result<Vec<ApiMarket>, ClobClientError> {
        let resp = self
            .authenticated_get("/markets")?
            .send()
            .await?
            .error_for_status()?;

        let markets: Vec<ApiMarket> = resp.json().await?;
        Ok(markets)
    }

    /// Fetch order book for a specific token.
    pub async fn get_order_book(
        &self,
        token_id: &str,
    ) -> Result<ApiOrderBook, ClobClientError> {
        let path = format!("/book?token_id={token_id}");
        let resp = self
            .authenticated_get(&path)?
            .send()
            .await?
            .error_for_status()?;

        let book: ApiOrderBook = resp.json().await?;
        Ok(book)
    }
}
