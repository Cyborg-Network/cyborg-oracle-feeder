mod account;
mod block_tracker;
mod builder;
/// The main function serves as the entry point for the Cyborg Client application.
/// It parses command-line arguments using Clap and executes the corresponding subcommand.
///
/// # Commands:
///
/// - `registration`: Registers a miner with the given blockchain node URL and account seed.
/// - `startmining`: Starts a mining session with the provided blockchain node URL, account seed, and IPFS URL.
///
/// # Errors:
///
/// Returns a `Box<dyn Error>` in case of failure, which could include errors from client building, registration, or mining operations.
///
/// # Usage:
///
/// Run the executable with appropriate subcommands to register or start mining a miner.
mod cli;
mod config;
mod error;
mod feeder;
mod integration_advanced;
mod integration_mock;
mod substrate_interface;
mod test_harness;
mod tx_queue;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::config::config;
use crate::tx_queue::init_transaction_queue;
use builder::CyborgOracleFeederBuilder;
use clap::Parser;
use cli::{Cli, Commands};
use feeder::OracleFeeder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    // Initialize transaction queue
    init_transaction_queue();

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
                .keypair(account_seed)
                .expect("Failed to set keypair")
                .build()
                .await?;

            let feeder = Arc::new(feeder);

            let feeder_for_miners = Arc::clone(&feeder);

            // Spawn async miner check loop
            let miners = tokio::spawn(async move { feeder_for_miners.run_check_miners().await });

            miners.await??;
        }
        _ => {
            println!("No command provided. Exiting.");
        }
    }

    Ok(())
}
