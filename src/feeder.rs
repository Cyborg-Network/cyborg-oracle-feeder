use crate::{
    block_tracker::BlockTracker,
    error::Error,
    substrate_interface::api::runtime_types::cyborg_primitives::oracle::{OracleKey, OracleValue},
    tx_queue::{TxOutput, TRANSACTION_QUEUE},
};
use async_trait::async_trait;
use reqwest::Client;
use subxt_signer::sr25519::Keypair;
use tokio::{
    sync::{Mutex, RwLock},
    time::{sleep, /*Instant, */ Duration},
};
//use rand::rngs::StdRng;
//use rand::{Rng, SeedableRng};
use crate::account::load_cyborg_test_key;
use crate::config::CLIENT;
use crate::substrate_interface::api::{
    self as SubstrateApi,
    runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleMinerFormat, ProcessStatus},
        },
    },
};
use serde::Deserialize;
use serde_aux::prelude::deserialize_bool_from_anything;
use std::sync::Arc;
pub struct SharedState {
    pub current_miners_data: Mutex<Option<Vec<(OracleKey, OracleValue)>>>,
}

#[allow(dead_code)]
pub struct CyborgOracleFeeder {
    pub keypair: Arc<RwLock<Keypair>>,
    pub shared_state: Arc<SharedState>,
    pub block_tracker: Arc<BlockTracker>,
}

// Subxt doesn't derive these, so I am writing a custom derive here
impl Clone for ProcessStatus {
    fn clone(&self) -> Self {
        ProcessStatus {
            available: self.available,
            online: self.online,
        }
    }
}
impl Clone for MinerType {
    fn clone(&self) -> Self {
        match self {
            MinerType::Cloud => MinerType::Cloud,
            MinerType::Edge => MinerType::Edge,
        }
    }
}
impl Clone for OracleMinerFormat {
    fn clone(&self) -> Self {
        OracleMinerFormat {
            id: self.id.clone(),
            miner_type: self.miner_type.clone(),
        }
    }
}
impl Clone for OracleKey {
    fn clone(&self) -> Self {
        match self {
            Self::Miner(miner) => Self::Miner(miner.clone()),
            Self::NzkProofResult(task_id) => Self::NzkProofResult(*task_id),
        }
    }
}
impl Clone for OracleValue {
    fn clone(&self) -> Self {
        match self {
            Self::MinerStatus(process_status) => Self::MinerStatus(process_status.clone()),
            Self::ZkProofResult(result) => Self::ZkProofResult(*result),
        }
    }
}

#[derive(Deserialize)]
struct MinerHealthResponse {
    #[serde(rename = "isActive", deserialize_with = "deserialize_bool_from_anything")]
    is_active: bool,
}

#[async_trait]
/// A trait for oracle feeder operations, such as getting the miners from the chain, the status of a single miner from the miner itself and feeding the oracle
///
/// Provides an asynchronous API which enables the feeding process
pub trait OracleFeeder {
    /// Runs the miner checking side of the oracle feeder, then waits some time before running it again.
    async fn run_check_miners(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Sets the miners that are currently registered onchain to self.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if successful, or an `Error` if the operation fails.
    async fn collect_miner_data(&self) -> Result<(), subxt::Error>;

    /// Attempts to feed the oracle with the values that were gathered from the miners at that point.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if the oracle was fed successfully, or an `Error` if it fails.
    async fn feed(&self) -> Result<(), Box<dyn std::error::Error>>;

    /// Collects status data from the miners and mutates `self.current_miners` accordingly.
    ///
    /// # Returns
    /// An `Option<String>` containing relevant information derived from the event, or `None` if no information is extracted.
    async fn get_miner_data(&self, miner_ip: &str, reqwest_client: &Client) -> ProcessStatus;
}

/// Implementation of the `OracleFeeder` trait for `CyborgOracleFeeder`.
#[async_trait]
impl OracleFeeder for CyborgOracleFeeder {
    async fn run_check_miners(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            println!("Running Oracle Feeder");
            println!("Starting new hour cycle...");

            /* // Record the starting time of the cycle
            let start_time = Instant::now();
            let one_hour = Duration::from_secs(3600);

            // Generate a random delay between 1 and 59 minutes
            let mut rng = StdRng::from_entropy();
            let random_delay_minutes = rng.gen_range(1..=59);
            let random_delay = Duration::from_secs(random_delay_minutes as u64 * 60);

            println!(
                "Random delay selected: {} minutes. Waiting...",
                random_delay_minutes
            ); */

            // This delay is only for testing and will be replaced by the one that's commented out
            let random_delay = Duration::from_secs(30);

            sleep(random_delay).await;

            if let Err(e) = self.collect_miner_data().await {
                println!("Failed to collect miner data: {}. Retrying next cycle.", e);
                continue;
            }

            self.feed().await.unwrap_or_else(|e| {
                println!("Failed to feed the oracle due to error: {e}. retrying in next cycle.")
            });

            /*             // Wait for the remainder of the hour
            let elapsed_time = start_time.elapsed();
            if elapsed_time < one_hour {
                let remaining_time = one_hour - elapsed_time;
                println!(
                    "Waiting for the remainder of the hour: {} seconds...",
                    remaining_time.as_secs()
                );
                sleep(remaining_time).await;
            } */

            println!("Hour cycle complete. Restarting...");
        }
    }

    async fn collect_miner_data(&self) -> Result<(), subxt::Error> {
        let mut new_miner_data: Vec<(OracleKey, OracleValue)> = Vec::new();

        let cloud_miners_address = SubstrateApi::storage().edge_connect().cloud_miners_iter();

        let edge_miners_address = SubstrateApi::storage().edge_connect().edge_miners_iter();

        let parachain_client = CLIENT.get().ok_or("Failed to get client")?;
        let reqwest_client = Client::new();

        let mut cloud_miners_query = parachain_client
            .storage()
            .at_latest()
            .await?
            .iter(cloud_miners_address)
            .await?;

        let mut edge_miners_query = parachain_client
            .storage()
            .at_latest()
            .await?
            .iter(edge_miners_address)
            .await?;

        println!("Collecting miner data...");

        while let Some(Ok(miner)) = cloud_miners_query.next().await {
            let miner_ip = String::from_utf8_lossy(&miner.value.api.domain.0).to_string();

            println!("Miner IP: {}", miner_ip);

            let process_status = self.get_miner_data(&miner_ip, &reqwest_client).await;

            new_miner_data.push((
                OracleKey::Miner(OracleMinerFormat {
                    id: miner.value.id,
                    miner_type: MinerType::Edge,
                }),
                OracleValue::MinerStatus(process_status),
            ));
        }

        while let Some(Ok(miner)) = edge_miners_query.next().await {
            let miner_ip = String::from_utf8_lossy(&miner.value.api.domain.0).to_string();

            println!("Miner IP: {}", miner_ip);

            let process_status = self.get_miner_data(&miner_ip, &reqwest_client).await;

            new_miner_data.push((
                OracleKey::Miner(OracleMinerFormat {
                    id: miner.value.id,
                    miner_type: MinerType::Edge,
                }),
                OracleValue::MinerStatus(process_status),
            ));
        }

        let mut miner_data_guard = self.shared_state.current_miners_data.lock().await;

        *miner_data_guard = Some(new_miner_data);

        Ok(())
    }

    async fn get_miner_data(&self, miner_ip: &str, reqwest_client: &Client) -> ProcessStatus {
        async fn process_response(
            response: reqwest::Response,
        ) -> Result<MinerHealthResponse, Box<dyn std::error::Error>> {
            let response_text = response.text().await?;
            println!("Response text: {}", response_text);
            let miner_health_item = serde_json::from_str::<MinerHealthResponse>(&response_text)?;

            Ok(miner_health_item)
        }

        let url = if miner_ip.starts_with("http://") || miner_ip.starts_with("https://") {
            format!("{}:8080/check-health", miner_ip)
        } else {
            format!("http://{}:8080/check-health", miner_ip)
        };
    
        println!("Attempting to connect to: {}", url);

        let response = reqwest_client
            .get(format!("{}:8080/check-health", miner_ip))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(response) => {
                println!("Response: {:?}", response);
                if let Ok(miner_health_item) = process_response(response).await {
                    println!(
                        "Miner with ip {} is online: {}",
                        miner_ip, miner_health_item.is_active
                    );
                    if miner_health_item.is_active {
                        ProcessStatus {
                            online: true,
                            available: true,
                        }
                    } else {
                        ProcessStatus {
                            online: false,
                            available: false,
                        }
                    }
                } else {
                    println!("Miner with IP {} returned an error", miner_ip);
                    ProcessStatus {
                        online: false,
                        available: false,
                    }
                }
            }
            Err(error) => {
                println!("Miner with ip {} is not online. Error: {}", miner_ip, error);
                ProcessStatus {
                    online: false,
                    available: false,
                }
            }
        }
    }

    async fn feed(&self) -> Result<(), Box<dyn std::error::Error>> {
        let lock = self.shared_state.current_miners_data.lock().await;
        if let Some(miners_data) = lock.as_ref() {
            let miners_data = miners_data.clone();

            // Get the transaction queue
            let queue = TRANSACTION_QUEUE
                .get()
                .ok_or("Transaction queue not initialized")?;

            // Enqueue the feed operation
            let rx = queue
                .enqueue(move || {
                    let miners_data = miners_data.clone();
                    async move {
                        let client = CLIENT
                            .get()
                            .ok_or_else(|| Error::custom("Failed to get client"))?;
                        let keypair = load_cyborg_test_key()?;

                        let feed_oracle_tx = SubstrateApi::tx()
                            .oracle()
                            .feed_values(BoundedVec(miners_data));

                        log::info!(
                            "Feed Oracle Miner Status Parameters: {:?}",
                            feed_oracle_tx.call_data()
                        );

                        let _ = client
                            .tx()
                            .sign_and_submit_then_watch_default(&feed_oracle_tx, &keypair)
                            .await
                            .map_err(|e| {
                                log::error!("Failed to submit transaction: {}", e);
                                e
                            })
                            .map_err(|e| Error::custom(e.to_string()))?
                            .wait_for_finalized_success()
                            .await
                            .map_err(|e| Error::custom(e.to_string()))?;

                        Ok(TxOutput::OracleFeedSuccess)
                    }
                })
                .await?;

            // Wait for the transaction to complete
            rx.await??;
            Ok(())
        } else {
            log::warn!("No miner data available to feed.");
            Err("No miner data available to feed.".into())
        }
    }
}
