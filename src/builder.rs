use std::sync::Arc;
use std::{path::PathBuf, str::FromStr};
use subxt_signer::{sr25519::Keypair as SR25519Keypair, SecretUri};
use tokio::sync::{Mutex, RwLock};

use crate::block_tracker::BlockTracker;
use crate::config::CLIENT;
use crate::feeder::{CyborgOracleFeeder, SharedState};

pub struct NoKeypair;
pub struct AccountKeypair(SR25519Keypair);

/// A builder pattern for constructing a `CyborgOracleFeeder` instance.
///
/// This builder allows for flexible configuration of the instance,
/// including setting the parachain URL and the oracle feeder account keypair.
pub struct CyborgOracleFeederBuilder<Keypair> {
    pub keypair: Keypair,
    shared_state: SharedState,
    data_dir: PathBuf,
}

/// Default implementation for the `CyborgOracleFeederBuilder` when no keypair is provided.
///
/// This initializes the builder with default values where parachain URL None
/// and the keypair is set to `NoKeypair`.
impl Default for CyborgOracleFeederBuilder<NoKeypair> {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cyborg-oracle-feeder");

        CyborgOracleFeederBuilder {
            keypair: NoKeypair,
            shared_state: SharedState {
                current_miners_data: Mutex::new(None),
            },
            data_dir,
        }
    }
}

impl<Keypair> CyborgOracleFeederBuilder<Keypair> {
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
        let uri = SecretUri::from_str(seed).expect("Keypair was not set correctly");
        let keypair = SR25519Keypair::from_uri(&uri).expect("Keypair from URI failed");

        Ok(CyborgOracleFeederBuilder {
            keypair: AccountKeypair(keypair),
            shared_state: self.shared_state,
            data_dir: self.data_dir,
        })
    }
}

impl CyborgOracleFeederBuilder<AccountKeypair> {
    /// Builds the `CyborgOracleFeeder` using the provided configurations.
    ///
    /// # Returns
    /// A `Result` that, if successful, contains the constructed `CyborgOracleFeeder`.
    pub async fn build(
        self,
    ) -> Result<CyborgOracleFeeder, Box<dyn std::error::Error + Send + Sync>> {
        let client = CLIENT.get().ok_or("Failed to get client")?;
        let block_tracker = BlockTracker::new(client.clone(), self.data_dir)?;

        Ok(CyborgOracleFeeder {
            keypair: Arc::new(RwLock::new(self.keypair.0)),
            shared_state: Arc::new(self.shared_state),
            block_tracker: Arc::new(block_tracker),
        })
    }
}
