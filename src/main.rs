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
mod substrate_interface;
mod builder;
mod feeder;

use builder::CyborgOracleFeederBuilder;
use clap::Parser;
use cli::{Cli, Commands};
use feeder::OracleFeeder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Match on the provided subcommand and execute the corresponding action.
    match &cli.command {
        // Handle the "start" subcommand.
        Some(Commands::Start {
            parachain_url,
            account_seed,
        }) => {
            println!("Starting the oracle feeder. Parachain URL: {}", parachain_url);

            // Build the oracle feeder using the provided parachain URL and account seed.
            let mut feeder = CyborgOracleFeederBuilder::default()
                .parachain_url(parachain_url.to_string())
                .keypair(account_seed)?
                .build()
                .await?;

            // Start the feeder.
            feeder.run_feeder().await?;
        }

        _ => {
            println!("No command provided. Exiting.");
        }
    }
    Ok(())
}