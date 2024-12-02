use subxt_signer::sr25519::Keypair;
use tokio::time::{sleep, Instant, Duration};
use async_trait::async_trait;
use subxt::{
    utils::AccountId32,
    OnlineClient, 
    PolkadotConfig,
};
use rand::Rng;
use crate::substrate_interface::api::{self as SubstrateApi, runtime_types::{bounded_collections::bounded_vec::BoundedVec, cyborg_primitives::{oracle::ProcessStatus, worker}}};

pub struct CyborgOracleFeeder {
    pub client: OnlineClient<PolkadotConfig>,
    pub parachain_url: String,
    pub keypair: Keypair,
    pub current_workers: Option<Vec<u64>>,
}

struct CyborgWorker {
    worker_id: (AccountId32, u32),
    worker_ip: String,
    status: ProcessStatus,
}

#[async_trait]
/// A trait for oracle feeder operations, such as getting the workers from the chain, the status of a single worker from the worker itself and feeding the oracle
///
/// Provides an asynchronous API which enables the feeding process
pub trait OracleFeeder {
    /// Runs the oracle feeder, then waits some time before running it again.
    async fn run_feeder(&mut self);

    /// Sets the workers that are currently registered onchain to self.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if successful, or an `Error` if the operation fails.
    async fn get_registered_workers(&mut self) -> Result<(), subxt::Error>;

    /// Attempts to feed the oracle with the values that were gathered from the workers at that point.
    ///
    /// # Returns
    /// A `Result` indicating `Ok(())` if the oracle was fed successfully, or an `Error` if it fails.
    async fn feed(&self) -> Result<(), subxt::Error>;

    /// Collects status data from the workers and mutates `self.current_workers` accordingly..
    ///
    /// # Returns
    /// An `Option<String>` containing relevant information derived from the event, or `None` if no information is extracted.
    async fn collect_worker_data(&mut self) -> Result<bool, reqwest::Error>;
}

/// Implementation of the `BlockchainClient` trait for `CyborgClient`.
#[async_trait]
impl OracleFeeder for CyborgOracleFeeder {
    async fn run_feeder(&mut self) -> Result<(), Box<dyn std::error::Error>> {
       loop {
            println!("Running Oracle Feeder");
            println!("Starting new hour cycle...");

             // Record the starting time of the cycle
            let start_time = Instant::now();
            let one_hour = Duration::from_secs(3600);

            // Generate a random delay between 1 and 59 minutes
            let mut rng = rand::thread_rng();
            let random_delay_minutes = rng.gen_range(1..=59);
            let random_delay = Duration::from_secs(random_delay_minutes as u64 * 60);

            println!(
                "Random delay selected: {} minutes. Waiting...",
                random_delay_minutes
            );

            sleep(random_delay).await;

            self.get_registered_workers().await.unwrap();

            self.collect_worker_data().await.unwrap();
           
            if let Some(current_workers) = &self.current_workers {
                for worker_id in current_workers {
                    let worker_ip = self.get_worker_ip(worker_id).await.unwrap();
                    self.update_worker_status(&worker_ip).await.unwrap();
                }
                if current_workers.len() == 0 {
                    println!("No workers registered");
                    break;
                }
            }

            self.feed().await.unwrap();

            // Wait for the remainder of the hour
            let elapsed_time = start_time.elapsed();
            if elapsed_time < one_hour {
                let remaining_time = one_hour - elapsed_time;
                println!(
                    "Waiting for the remainder of the hour: {} seconds...",
                    remaining_time.as_secs()
                );
                sleep(remaining_time).await;
            }

            println!("Hour cycle complete. Restarting...");
       } 

        Ok(())
    }

    async fn get_registered_workers(
        &mut self, 
    ) -> Result<(), subxt::Error> {
        let mut workers: Vec<CyborgWorker> = Vec::new();

        let workers_address =
            SubstrateApi::storage().edge_connect().executable_workers_iter();
        
        let workers_query = self.client
            .storage()
            .at_latest()
            .await?
            .fetch(&workers_address)
            .await?
            .iter();

        while let Some(worker) = workers_query.next().await? {
            workers.push(worker);
        }

        Ok(())
    }

    async fn collect_worker_data(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(current_workers) = &self.current_workers {
            if current_workers.len() > 0 {
                for worker in current_workers{
                    // TODO insert worker ip from worker from list
                    let response = reqwest::get(format!("http://{}:8080/check_health", worker_ip)).await?;

                    // TODO check response for actual content
                    if response.status().is_success() {
                        println!("Worker {} is healthy", worker_ip);
                    }
                    // TODO set worker field to healthy or unhealthy depending on response

                    // TODO keep list of workers that errored instead of sending a response back?
                }
            }
        }

        Ok(())
    }

    async fn feed(
        &self, 
    ) -> Result<(), subxt::Error> {
        let process_status = if value {
            SubstrateApi::runtime_types::cyborg_primitives::oracle::ProcessStatus{
                online: true,
                available: true
            }
        } else {
            SubstrateApi::runtime_types::cyborg_primitives::oracle::ProcessStatus{
                online: false,
                available: false
            }
        };

        let feed_oracle_tx = SubstrateApi::tx()
        .oracle()
        .feed_values(
            BoundedVec::from((worker_id, process_status))
        );

        println!("Feed Oracle Parameters: {:?}", feed_oracle_tx.call_data());

        let feed_oracle_events = self.client
            .tx()
            .sign_and_submit_then_watch_default(&feed_oracle_tx, &self.keypair)
            .await
            .map(|e| {
                println!("Values submitted to oracle, waiting for transaction to be finalized...");
                e
            })?
            .wait_for_finalized_success()
            .await?;

        let feed_event = 
            feed_oracle_events.find_first::<SubstrateApi::task_management::events::SubmittedCompletedTask>()?;
        if let Some(event) = feed_event {
            println!("Oracle fed successfully: {event:?}");
        } else {
            println!("Failed find event corresponding to feeding process.");
        }

        Ok(())
    }
}