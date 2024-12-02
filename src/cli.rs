use clap::{Parser, Subcommand};

#[derive(Debug, Parser, PartialEq)]
#[command(
    name = "cyborg-oracle-feeder", // Name of the CLI tool.
    about = "Am executable that will query the available workers and submit the results to the oracle.", // Description shown in the CLI help.
    version = "1.0" // Version number of the CLI tool.
)]

/// `Cli` struct defines the command-line interface for the Cyborg worker.
/// This struct uses the `clap` crate to parse command-line arguments.
/// It contains a single field `command` which specifies the subcommand to be executed.
pub struct Cli {
    /// Specify the subcommand to run.
    #[command(subcommand)]
    pub command: Option<Commands>, // Defines the possible subcommands, wrapped in an `Option`.
}

// Enum to define the available subcommands. Each variant corresponds to a different command.
#[derive(Debug, Subcommand, PartialEq)]
/// `Commands` enum defines the available subcommands for the Cyborg worker.
/// Each variant represents a specific action that can be performed by the worker.
/// - `Start`: Starts the feeder with the provided parachain URL and feeder account seed.
pub enum Commands {
    /// Start the oracle feeder.
    Start {
        /// API URL for the registration process.
        #[clap(long, value_name = "PARACHAIN_URL")]
        parachain_url: String,

        /// Account Seed for the oracle feeder. Needs to be a registered as an oracle feeder on the cyborg parachain.
        #[clap(long, value_name = "ACCOUNT_SEED")]
        account_seed: String,
    },
}