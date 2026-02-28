use std::str::FromStr;

use alloy::signers::local::PrivateKeySigner;
use polymarket_client_sdk::auth::Signer;
use polymarket_client_sdk::clob::client::{Client, Config};
use polymarket_client_sdk::POLYGON;

/// Wraps the authenticated Polymarket SDK client and signer.
///
/// The private key is used once during construction and never stored as a string.
pub struct PolymarketWallet {
    signer: PrivateKeySigner,
    client: Client<polymarket_client_sdk::auth::state::Authenticated<polymarket_client_sdk::auth::Normal>>,
}

impl PolymarketWallet {
    /// Create a new wallet from a hex-encoded private key (with or without `0x` prefix).
    ///
    /// This authenticates against the Polymarket CLOB API, deriving or creating
    /// an API key as needed.
    pub async fn new(private_key: &str) -> anyhow::Result<Self> {
        let signer = PrivateKeySigner::from_str(private_key)?
            .with_chain_id(Some(POLYGON));

        let config = Config::default();
        let unauthenticated = Client::new("https://clob.polymarket.com", config)?;

        let client = unauthenticated
            .authentication_builder(&signer)
            .authenticate()
            .await?;

        Ok(Self { signer, client })
    }

    /// Return the wallet's Ethereum address as a checksummed hex string.
    pub fn wallet_address(&self) -> String {
        format!("{}", self.client.address())
    }

    /// Borrow the authenticated SDK client.
    pub fn client(
        &self,
    ) -> &Client<polymarket_client_sdk::auth::state::Authenticated<polymarket_client_sdk::auth::Normal>>
    {
        &self.client
    }

    /// Borrow the local signer (needed for order signing).
    pub fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }
}
