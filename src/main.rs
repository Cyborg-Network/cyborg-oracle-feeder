mod builder;
/// The main function serves as the entry point for the Cyborg Client application.
/// It parses command-line arguments using Clap and executes the corresponding subcommand.
///
/// # Commands:
///
/// - `registration`: Registers a worker with the given blockchain node URL and account seed.
/// - `startmining`: Starts a mining session with the provided blockchain node URL, account seed, and IPFS URL.
///
/// # Errors:
///
/// Returns a `Box<dyn Error>` in case of failure, which could include errors from client building, registration, or mining operations.
///
/// # Usage:
///
/// Run the executable with appropriate subcommands to register or start mining a worker.
mod cli;
mod feeder;
mod substrate_interface;
mod config;
mod error;
mod account;

use std::sync::Arc;

use builder::CyborgOracleFeederBuilder;
use clap::Parser;
use cli::{Cli, Commands};
use feeder::OracleFeeder;

use crate::config::{config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Start {
            parachain_url,
            account_seed,
        }) => {
            println!(
                "Starting the oracle feeder. Parachain URL: {}",
                parachain_url
            );

            // Initialize config + global client/keypair once
            config(parachain_url).await;

            // Build feeder
            let feeder = CyborgOracleFeederBuilder::default()
                .keypair(&account_seed).expect("Failed to set keypair")
                .build()
                .await;

            let feeder = Arc::new(feeder);

            // Clone for parallel tasks
            let feeder_for_proofs = Arc::clone(&feeder);
            let feeder_for_workers = Arc::clone(&feeder);

            // Spawn verification in blocking thread
            let proofs = tokio::task::spawn_blocking(move || {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(feeder_for_proofs.run_verify_proofs())
            });

            // Spawn async worker check loop
            let workers = tokio::spawn(async move {
                feeder_for_workers.run_check_workers().await
            });

            // Wait for both tasks
            let (proofs_result, workers_result) = tokio::join!(proofs, workers);

            workers_result??;
            proofs_result??;
        }
        _ => {
            println!("No command provided. Exiting.");
        }
    }

    Ok(())
}
