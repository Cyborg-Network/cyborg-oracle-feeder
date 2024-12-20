use subxt_signer::sr25519::Keypair;
use tokio::time::{sleep, Instant, Duration};
use async_trait::async_trait;
use subxt::{
    utils::AccountId32,
    OnlineClient, 
    PolkadotConfig,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use crate::substrate_interface::api::edge_connect::storage::types::executable_workers;
use crate::substrate_interface::api::{
    self as SubstrateApi, 
    runtime_types::{
        bounded_collections::bounded_vec::BoundedVec, 
        cyborg_primitives::{worker::WorkerType, oracle::{ProcessStatus, OracleWorkerFormat}}
    }
};
use serde::Deserialize;
use serde_aux::prelude::deserialize_bool_from_anything;

pub struct CyborgOracleFeeder {
    pub client: OnlineClient<PolkadotConfig>,
    pub keypair: Keypair,
    pub current_workers_data: Option<Vec<CyborgWorkerData>>,
}

// For some reason subxt doesn't implement the Clone trait for ProcessStatus, so I am adding it manually here to avoid messing with the codegen
impl Clone for ProcessStatus {
    fn clone(&self) -> Self {
        ProcessStatus{
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
        OracleWorkerFormat{
            id: self.id.clone(), 
            worker_type: self.worker_type.clone(),
        }
    }
}


type CyborgWorkerData = (OracleWorkerFormat<AccountId32>, ProcessStatus);

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
    /// Runs the oracle feeder, then waits some time before running it again.
    async fn run_feeder(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    /// Sets the workers that are currently registered onchain to self.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if successful, or an `Error` if the operation fails.
    async fn collect_worker_data(&mut self) -> Result<(), subxt::Error>;

    /// Attempts to feed the oracle with the values that were gathered from the workers at that point.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if the oracle was fed successfully, or an `Error` if it fails.
    async fn feed(&self) -> Result<(), Box<dyn std::error::Error>>;

    /// Collects status data from the workers and mutates `self.current_workers` accordingly..
    ///
    /// # Returns
    /// An `Option<String>` containing relevant information derived from the event, or `None` if no information is extracted.
    async fn get_worker_data(&mut self, worker_ip: &String) -> ProcessStatus;
}

/// Implementation of the `BlockchainClient` trait for `CyborgClient`.
#[async_trait]
impl OracleFeeder for CyborgOracleFeeder {
    async fn run_feeder(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
           
            self.feed().await.unwrap_or_else(|e| println!("Failed to feed the oracle due to error: {e}. retrying in next cycle."));

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

    async fn collect_worker_data(
        &mut self, 
    ) -> Result<(), subxt::Error> {
        let mut worker_data: Vec<CyborgWorkerData> = Vec::new();

        let worker_clusters_address =
            SubstrateApi::storage().edge_connect().worker_clusters_iter();
        
        let executable_workers_address =
            SubstrateApi::storage().edge_connect().executable_workers_iter();
        
        let mut worker_clusters_query = self.client
            .storage()
            .at_latest()
            .await?
            .iter(worker_clusters_address)
            .await?;

        let mut executable_workers_query = self.client
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
                OracleWorkerFormat{
                    id: (worker.value.owner, worker.value.id),
                    worker_type: WorkerType::Docker,
                },
                process_status
            ));
        }

        while let Some(Ok(worker)) = executable_workers_query.next().await {
            let worker_ip = String::from_utf8_lossy(&worker.value.api.domain.0).to_string();

            println!("Worker IP: {}", worker_ip);

            let process_status = self.get_worker_data(&worker_ip).await;

            worker_data.push((
                OracleWorkerFormat{
                    id: (worker.value.owner, worker.value.id),
                    worker_type: WorkerType::Executable,
                },
                process_status
            ));
        }

        self.current_workers_data = Some(worker_data);

        Ok(())
    }

    async fn get_worker_data(
        &mut self,
        worker_ip: &String,
    ) -> ProcessStatus {

        async fn process_response(response: reqwest::Response) -> Result<WorkerHealthResponse, Box<dyn std::error::Error>> {
            let response_text = response.text().await?;
            println!("Response text: {}", response_text);
            let worker_health_item = serde_json::from_str::<WorkerHealthResponse>(&response_text)?;

            Ok(worker_health_item)
        }
                    
        // TODO insert worker ip from worker from list
        let response = reqwest::get(format!("http://{}:8080/check-health", worker_ip)).await;

        match response {
            Ok(response) => {
                println!("Response: {:?}", response);
                if let Ok(worker_health_item) = process_response(response).await {
                    println!("Worker with ip {} is online: {}", worker_ip, worker_health_item.is_active);
                    if worker_health_item.is_active {
                        ProcessStatus{
                            online: true,
                            available: true
                        }   
                    } else {
                        ProcessStatus{
                            online: false,
                            available: false
                        }
                    }
                } else {
                    println!("Worker with IP {} returned an error", worker_ip);
                    ProcessStatus{
                        online: false,
                        available: false,
                    }
                }
            },
            Err(error) => {
                println!("Worker with ip {} is not online. Error: {}", worker_ip, error);
                ProcessStatus{
                    online: false,
                    available: false
                }
            }
        }
    }

    async fn feed(
        &self, 
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(workers_data) = &self.current_workers_data {

            // TODO: Since a bound vector is being submitted (and it also wouldn't make sense otherwise) the feeder can only cover a certain amount of workers. A mechanism needs to be implemented that disteributes workers to be checked between different oracle feeders.

            let feed_oracle_tx = SubstrateApi::tx()
                .oracle()
                .feed_values(
                    BoundedVec(workers_data.clone())
                );

            println!("Feed Oracle Parameters: {:?}", feed_oracle_tx.call_data());

            let _ = self.client
                .tx()
                .sign_and_submit_then_watch_default(&feed_oracle_tx, &self.keypair)
                .await
                .map(|e| {
                    println!("Values submitted to oracle, waiting for transaction to be finalized...");
                    e
                })?
                .wait_for_finalized_success()
                .await?;

            Ok(())
        } else {
            println!("No worker data available to feed.");
            Err("No worker data available to feed.".into())
        }
    }
}