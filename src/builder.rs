use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::{SecretUri, sr25519::Keypair as SR25519Keypair};
use std::str::FromStr;

use crate::feeder::CyborgOracleFeeder;

pub struct NoKeypair;
pub struct AccountKeypair(SR25519Keypair);

/// A builder pattern for constructing a `CyborgClient` instance.
///
/// This builder allows for flexible configuration of the Cyborg client,
/// including setting the node URI, keypair, and IPFS URI.
pub struct CyborgOracleFeederBuilder<Keypair> {
    parachain_url: Option<String>,
    keypair: Keypair,
}

/// Default implementation for the `CyborgOracleFeederBuilder` when no keypair is provided.
///
/// This initializes the builder with default values where parachain URL None
/// and the keypair is set to `NoKeypair`.
impl Default for CyborgOracleFeederBuilder<NoKeypair> {
    fn default() -> Self {
        CyborgOracleFeederBuilder {
            parachain_url: None,
            keypair: NoKeypair,
        }
    }
}

impl<Keypair> CyborgOracleFeederBuilder<Keypair> {
    /// Sets the node URI for the client to connect to.
    ///
    /// # Arguments
    /// * `url` - A string representing the WebSocket URL of the node.
    pub fn parachain_url(mut self, url: String) -> Self {
        self.parachain_url = Some(url);
        self
    }

    /// Sets the keypair for the feeder using a provided seed phrase.
    ///
    /// # Arguments
    /// * `seed` - A string slice representing the seed phrase for generating the keypair.
    ///
    /// # Returns
    /// A `Result` that, if successful, contains a new `CyborgOracleFeederBuilder` instance with an `AccountKeypair`.
    pub fn keypair(
        self,
        seed: &str,
    ) -> Result<CyborgOracleFeederBuilder<AccountKeypair>, Box<dyn std::error::Error>> {
        println!("Keypair: {}", seed);
        let uri = SecretUri::from_str(seed)
            .expect("Keypair was not set correctly");
        let keypair = SR25519Keypair::from_uri(&uri)
            .expect("Keypair from URI failed");

        Ok(CyborgOracleFeederBuilder {
            parachain_url: self.parachain_url,
            keypair: AccountKeypair(keypair),
        })
    }
}

impl CyborgOracleFeederBuilder<AccountKeypair> {
    /// Builds the `CyborgOracleFeeder` using the provided configurations.
    ///
    /// # Returns
    /// A `Result` that, if successful, contains the constructed `CyborgOracleFeeder`.
    pub async fn build(self) -> Result<CyborgOracleFeeder, Box<dyn std::error::Error>> {
        match &self.parachain_url {
            Some(url) => {
                // Create an online client that connects to the specified Substrate node URL.
                let client = OnlineClient::<PolkadotConfig>::from_url(url).await?;

                Ok(CyborgOracleFeeder {
                    client,
                    keypair: self.keypair.0,
                    parachain_url: self.parachain_url
                        .expect("Parachain URL was not set, cannot run feeder without an endpoint connecting it to cyborg network."),
                    current_workers: None,
                })
            }
            None => Err("No node URI provided. Please specify a node URI to connect.".into()),
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use clap::builder;
    use sp_core::sr25519;
    use std::error::Error;

    #[tokio::test]
    async fn test_node_uri() {
        // Test setting the node URI in the builder.
        let builder = CyborgClientBuilder::default().node_uri("ws://127.0.0.1:9988".to_string());
        assert_eq!(builder.node_uri, Some("ws://127.0.0.1:9988".to_string()));

        // Test setting both node URI and keypair.
        let builder = CyborgClientBuilder::default()
            .node_uri("ws://127.0.0.1:9988".to_string())
            .keypair("//Alice");

        assert_eq!(
            builder.unwrap().node_uri,
            Some("ws://127.0.0.1:9988".to_string())
        );
    }

    #[tokio::test]
    async fn test_keypair() {
        // Test setting the keypair in the builder.
        let builder = CyborgClientBuilder::default()
            .node_uri("ws://127.0.0.1:9988".to_string())
            .keypair("//Alice");

        let uri_alice = SecretUri::from_str("//Alice").unwrap();
        let expected_public_key = SR25519Keypair::from_uri(&uri_alice)
            .expect("keypair was not set correctly")
            .public_key();
        let uri_bob = SecretUri::from_str("//Bob").unwrap();
        let unexpected_public_key = SR25519Keypair::from_uri(&uri_bob)
            .expect("keypair was not set correctly")
            .public_key();

        if let AccountKeypair(keypair) = builder.unwrap().keypair {
            assert_eq!(keypair.public_key().to_account_id(), expected_public_key.to_account_id());
        } else {
            assert_eq!(unexpected_public_key.to_account_id(), expected_public_key.to_account_id());
        }
    }

    #[tokio::test]
    async fn test_ipfs_uri() -> Result<(), Box<dyn Error>> {
        // Test setting the IPFS URI in the builder.
        let builder = CyborgClientBuilder::default()
            .node_uri("ws://127.0.0.1:9944".to_string())
            .keypair("//Alice")?
            .ipfs_uri("http://127.0.0.1:5001".to_string());

        assert_eq!(builder.ipfs_uri, Some("http://127.0.0.1:5001".to_string()));

        // Test setting the IPFS URI without a keypair.
        let builder = CyborgClientBuilder::default()
            .node_uri("ws://127.0.0.1:9944".to_string())
            .ipfs_uri("http://127.0.0.1:5001".to_string());

        assert_eq!(builder.ipfs_uri, Some("http://127.0.0.1:5001".to_string()));

        Ok(())
    }
}
*/