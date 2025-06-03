use crate::substrate_interface::api::runtime_types::cyborg_primitives::oracle::{
    OracleKey, OracleValue,
};
use async_trait::async_trait;
use ezkl::Commitments;
use reqwest::Client;
use subxt::{utils::AccountId32};
use subxt_signer::sr25519::Keypair;
use tokio::{sync::{Mutex, RwLock}, time::{sleep, /*Instant, */ Duration}};
//use rand::rngs::StdRng;
//use rand::{Rng, SeedableRng};
//use crate::substrate_interface::api::edge_connect::storage::types::executable_workers;
use crate::substrate_interface::{
    self,
    api::{
        self as SubstrateApi,
        runtime_types::{
            bounded_collections::bounded_vec::BoundedVec,
            cyborg_primitives::{
                oracle::{OracleWorkerFormat, ProcessStatus},
                worker::WorkerType,
            },
        },
    },
};
use ezkl::{
    commands::Commands::{GetSrs, Verify},
    execute::run,
};
use crate::config::{CLIENT};
use serde::Deserialize;
use serde_aux::prelude::deserialize_bool_from_anything;
use std::{fs::write, sync::Arc};
use tempfile::tempdir;
use crate::account::load_cyborg_test_key;

pub struct SharedState {
    pub current_workers_data: Mutex<Option<Vec<(OracleKey<AccountId32>, OracleValue)>>>,
}

pub struct CyborgOracleFeeder {
    pub keypair: Arc<RwLock<Keypair>>,
    pub shared_state: Arc<SharedState>,
}

// Subxt doesn't derive these, so I am writing a custom derive here
impl Clone for ProcessStatus {
    fn clone(&self) -> Self {
        ProcessStatus {
            available: self.available.clone(),
            online: self.online.clone(),
        }
    }
}
impl Clone for WorkerType {
    fn clone(&self) -> Self {
        match self {
            WorkerType::Docker => WorkerType::Docker,
            WorkerType::Executable => WorkerType::Executable,
        }
    }
}
impl Clone for OracleWorkerFormat<AccountId32> {
    fn clone(&self) -> Self {
        OracleWorkerFormat {
            id: self.id.clone(),
            worker_type: self.worker_type.clone(),
        }
    }
}
impl Clone for OracleKey<AccountId32> {
    fn clone(&self) -> Self {
        match self {
            Self::Miner(miner) => Self::Miner(miner.clone()),
            Self::NzkProofResult(task_id) => Self::NzkProofResult(task_id.clone()),
        }
    }
}
impl Clone for OracleValue {
    fn clone(&self) -> Self {
        match self {
            Self::MinerStatus(process_status) => Self::MinerStatus(process_status.clone()),
            Self::ZkProofResult(result) => Self::ZkProofResult(result.clone()),
        }
    }
}

#[derive(Deserialize)]
struct WorkerHealthResponse {
    #[serde(deserialize_with = "deserialize_bool_from_anything")]
    is_active: bool,
}

#[async_trait]
/// A trait for oracle feeder operations, such as getting the workers from the chain, the status of a single worker from the worker itself and feeding the oracle
///
/// Provides an asynchronous API which enables the feeding process
pub trait OracleFeeder {
    /// Runs the worker checking side of the oracle feeder, then waits some time before running it again.
    async fn run_check_workers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Runs the proof verification side of the oracle feeder.
    async fn run_verify_proofs(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Verifies a single proof
    async fn verify_proof(
        &self,
        task_id: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Sets the workers that are currently registered onchain to self.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if successful, or an `Error` if the operation fails.
    async fn collect_worker_data(&self) -> Result<(), subxt::Error>;

    /// Attempts to feed the oracle with the values that were gathered from the workers at that point.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if the oracle was fed successfully, or an `Error` if it fails.
    async fn feed(&self) -> Result<(), Box<dyn std::error::Error>>;

    /// Collects status data from the workers and mutates `self.current_workers` accordingly.
    ///
    /// # Returns
    /// An `Option<String>` containing relevant information derived from the event, or `None` if no information is extracted.
    async fn get_worker_data(&self, worker_ip: &String) -> ProcessStatus;
}

/// Implementation of the `OracleFeeder` trait for `CyborgOracleFeeder`.
#[async_trait]
impl OracleFeeder for CyborgOracleFeeder {
    async fn run_verify_proofs(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut blocks = CLIENT.get()
            .ok_or("Failed to get client")?
            .blocks()
            .subscribe_finalized()
            .await?;

        while let Some(Ok(block)) = blocks.next().await {
            println!("New block imported: {:?}", block.hash());

            let events = block.events().await?;

            for event in events.iter() {
                match event {
                    Ok(ev) => {
                        match ev.as_event::<substrate_interface::api::neuro_zk::events::NzkProofSubmitted>() {
                            Ok(Some(proof_event)) => {
                                let task_id = proof_event.task_id;
                                let submitting_miner = &proof_event.submitting_miner;

                                println!(
                                    "Processing proof: Task ID: {:?}, Submitting Miner: {:?}",
                                    task_id, submitting_miner
                                );

                                self.verify_proof(task_id).await?;
                            }
                            Err(e) => {
                                println!("Error decoding ProofSubmitted event: {:?}", e);
                                return Err(Box::new(e));
                            }
                            _ => {} // Skip non-matching events 
                        }
                    }
                    Err(e) => eprintln!("Error decoding event: {:?}", e),
                }
            }
        }

        Ok(())
    }

    async fn run_check_workers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

            self.collect_worker_data().await?;

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

    async fn verify_proof(
        &self,
        task_id: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Query the parachain storage map
        let client = CLIENT.get().ok_or("Failed to get client")?;
        let task_address = SubstrateApi::storage().task_management().tasks(task_id);
        let task = client
            .storage()
            .at_latest()
            .await?
            .fetch(&task_address)
            .await?;

        if let Some(task) = task {
            if let Some(nzk_data) = task.nzk_data {
                if let Some(proof) = nzk_data.zk_proof {
                    let dir = tempdir()?;
                    let proof_path = dir.path().join("proof.json");
                    let vk_path = dir.path().join("vk.key");
                    let srs_path = dir.path().join("kzg.srs");
                    let settings_path = dir.path().join("settings.json");

                    write(&proof_path, proof.0)?;
                    write(&vk_path, nzk_data.zk_verifying_key.0)?;
                    write(&settings_path, nzk_data.zk_settings.0)?;

                    let _ = run(GetSrs {
                        srs_path: Some(srs_path.clone()),
                        settings_path: Some(settings_path.clone()),
                        logrows: None,
                        commitment: Some(Commitments::KZG),
                    })
                    .await?;

                    //TODO obv it's nasty that this returns a string, but ezkl verification options will change with next release (verification via bytearrray), so this isn't final anyway
                    let string_result = run(Verify {
                        settings_path: Some(settings_path),
                        proof_path: Some(proof_path),
                        vk_path: Some(vk_path),
                        srs_path: Some(srs_path),
                        reduced_srs: None,
                    })
                    .await?;

                    println!("Verification result: {}", string_result);

                    let bool_result: bool;

                    match string_result.as_str() {
                        "true" => bool_result = true,
                        "false" => bool_result = false,
                        _ => return Err("Verification failed".into()),
                    };

                    let result: (OracleKey<AccountId32>, OracleValue) = (
                        OracleKey::NzkProofResult(task_id),
                        OracleValue::ZkProofResult(bool_result),
                    );

                    let result = vec![result];

                    let feed_oracle_tx = SubstrateApi::tx()
                        .oracle()
                        .feed_values(BoundedVec(result));

                    println!(
                        "Feed Oracle NeuroZk Parameters: {:?}",
                        feed_oracle_tx.call_data()
                    );

                    let keypair = load_cyborg_test_key()?;

                    let tx_progress = client
                        .tx()
                        .sign_and_submit_then_watch_default(&feed_oracle_tx, &keypair)
                        .await
                        .map_err(|e| {
                            eprintln!("Extrinsic submission failed: {:?}", e);
                            e
                        })?;

                    println!("Extrinsic submitted. Waiting for inclusion...");

                    let finalized = tx_progress.wait_for_finalized().await?;

                    println!("Extrinsic finalized in block: {:?}", finalized.block_hash());

                    let events = finalized.fetch_events().await?;

                    for evt in events.iter() {
                        match evt {
                            Ok(ev) => {
                                if let Some(system_event) = ev.as_event::<SubstrateApi::system::events::ExtrinsicFailed>()? {
                                    println!("Extrinsic failed with error: {:?}", system_event);
                                    println!("Dispatch error: {:?}", system_event.dispatch_error);
                                    return Err("Extrinsic failed on chain".into());
                                } else if let Some(_) = ev.as_event::<SubstrateApi::system::events::ExtrinsicSuccess>()? {
                                    println!("Extrinsic succeeded!");
                                }
                            }
                            Err(e) => {
                                println!("Error parsing event: {:?}", e);
                            }
                        }
                    }

                    Ok(())
                } else {
                    Err("Zk proof not found".into())
                }
            } else {
                Err("Nzk data not found".into())
            }
        } else {
            Err("Task not found".into())
        }
    }

    async fn collect_worker_data(&self) -> Result<(), subxt::Error> {
        let mut worker_data: Vec<(OracleKey<AccountId32>, OracleValue)> = Vec::new();

        let worker_clusters_address = SubstrateApi::storage()
            .edge_connect()
            .worker_clusters_iter();

        let executable_workers_address = SubstrateApi::storage()
            .edge_connect()
            .executable_workers_iter();

        let client = CLIENT.get().ok_or("Failed to get client")?;

        let mut worker_clusters_query = client
            .storage()
            .at_latest()
            .await?
            .iter(worker_clusters_address)
            .await?;

        let mut executable_workers_query = client
            .storage()
            .at_latest()
            .await?
            .iter(executable_workers_address)
            .await?;

        println!("Collecting worker data...");

        while let Some(Ok(worker)) = worker_clusters_query.next().await {
            let worker_ip = String::from_utf8_lossy(&worker.value.api.domain.0).to_string();

            println!("Worker IP: {}", worker_ip);

            let process_status = self.get_worker_data(&worker_ip).await;

            worker_data.push((
                OracleKey::Miner(OracleWorkerFormat {
                    id: (worker.value.owner, worker.value.id),
                    worker_type: WorkerType::Executable,
                }),
                OracleValue::MinerStatus(process_status),
            ));
        }

        while let Some(Ok(worker)) = executable_workers_query.next().await {
            let worker_ip = String::from_utf8_lossy(&worker.value.api.domain.0).to_string();

            println!("Miner IP: {}", worker_ip);

            let process_status = self.get_worker_data(&worker_ip).await;

            worker_data.push((
                OracleKey::Miner(OracleWorkerFormat {
                    id: (worker.value.owner, worker.value.id),
                    worker_type: WorkerType::Executable,
                }),
                OracleValue::MinerStatus(process_status),
            ));
        }

        let mut miner_data = 
            self.shared_state.current_workers_data.lock().await;

        *miner_data = Some(worker_data);

        Ok(())
    }

    async fn get_worker_data(&self, worker_ip: &String) -> ProcessStatus {
        async fn process_response(
            response: reqwest::Response,
        ) -> Result<WorkerHealthResponse, Box<dyn std::error::Error>> {
            let response_text = response.text().await?;
            println!("Response text: {}", response_text);
            let worker_health_item = serde_json::from_str::<WorkerHealthResponse>(&response_text)?;

            Ok(worker_health_item)
        }

        let client = Client::new();
        let response = client
            .get(format!("http://{}:8080/check-health", worker_ip))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(response) => {
                println!("Response: {:?}", response);
                if let Ok(worker_health_item) = process_response(response).await {
                    println!(
                        "Miner with ip {} is online: {}",
                        worker_ip, worker_health_item.is_active
                    );
                    if worker_health_item.is_active {
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
                    println!("Miner with IP {} returned an error", worker_ip);
                    ProcessStatus {
                        online: false,
                        available: false,
                    }
                }
            }
            Err(error) => {
                println!(
                    "Miner with ip {} is not online. Error: {}",
                    worker_ip, error
                );
                ProcessStatus {
                    online: false,
                    available: false,
                }
            }
        }
    }

    async fn feed(&self) -> Result<(), Box<dyn std::error::Error>> {
        let lock = self.shared_state.current_workers_data.lock().await;
        if let Some(workers_data) = lock.as_ref() {
            // TODO: Since a bound vector is being submitted (and it also wouldn't make sense otherwise) the feeder can only cover a certain amount of workers. A mechanism needs to be implemented that disteributes workers to be checked between different oracle feeders.

            let feed_oracle_tx = SubstrateApi::tx()
                .oracle()
                .feed_values(BoundedVec(workers_data.clone()));

            println!(
                "Feed Oracle Miner Status Parameters: {:?}",
                feed_oracle_tx.call_data()
            );

            let client = CLIENT.get().ok_or("Failed to get client")?;
            let keypair = load_cyborg_test_key()?;
            let _ = client
                .tx()
                .sign_and_submit_then_watch_default(&feed_oracle_tx, &keypair)
                .await
                .map(|e| {
                    println!(
                        "Values submitted to oracle, waiting for transaction to be finalized..."
                    );
                    e
                })?
                .wait_for_finalized_success()
                .await?;

            Ok(())
        } else {
            println!("No miner data available to feed.");
            Err("No miner data available to feed.".into())
        }
    }
}
