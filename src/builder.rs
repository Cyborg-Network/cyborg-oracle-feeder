use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::{SecretUri, sr25519::Keypair as SR25519Keypair};
use std::str::FromStr;

use crate::feeder::CyborgOracleFeeder;

pub struct NoKeypair;
pub struct AccountKeypair(SR25519Keypair);

/// A builder pattern for constructing a `CyborgOracleFeeder` instance.
///
/// This builder allows for flexible configuration of the instance,
/// including setting the parachain URL and the oracle feeder account keypair.
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
    /// Sets the parachain URL for the feeder to connect to.
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
                    current_workers_data: None,
                })
            }
            None => Err("No node URI provided. Please specify a node URI to connect.".into()),
        }
    }
}