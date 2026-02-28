use base64::{
    engine::general_purpose::{STANDARD as BASE64, URL_SAFE as BASE64_URL_SAFE},
    Engine,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid base64 secret: {0}")]
    InvalidSecret(#[from] base64::DecodeError),

    #[error("HMAC computation failed: {0}")]
    HmacError(String),
}

#[derive(Debug, Clone)]
pub struct PolymarketAuth {
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: String,
}

impl PolymarketAuth {
    pub fn new(api_key: String, api_secret: String, passphrase: String) -> Self {
        Self {
            api_key,
            api_secret,
            passphrase,
        }
    }

    /// Build HMAC-SHA256 signature for Polymarket CLOB API.
    ///
    /// message = `{timestamp}{method}{path}{body}`
    /// secret is base64-decoded before use.
    pub fn sign(
        &self,
        timestamp: &str,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<String, AuthError> {
        // Polymarket API secrets use URL-safe base64 (with - and _)
        let secret_bytes = BASE64_URL_SAFE
            .decode(&self.api_secret)
            .or_else(|_| BASE64.decode(&self.api_secret))?;

        let message = format!("{timestamp}{method}{path}{body}");

        let mut mac = HmacSha256::new_from_slice(&secret_bytes)
            .map_err(|e| AuthError::HmacError(e.to_string()))?;

        mac.update(message.as_bytes());
        let result = mac.finalize();

        Ok(BASE64.encode(result.into_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_produces_base64_output() {
        // Use a known base64-encoded secret
        let secret = BASE64.encode(b"test-secret-key-1234");
        let auth = PolymarketAuth::new("key".into(), secret, "pass".into());

        let sig = auth.sign("1700000000", "GET", "/markets", "").unwrap();

        // Verify the signature is valid base64
        assert!(BASE64.decode(&sig).is_ok());
        // Signature should be 44 chars (32 bytes base64-encoded)
        assert_eq!(sig.len(), 44);
    }
}
