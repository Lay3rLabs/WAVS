use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    #[clap(long, default_value = "ws://localhost:8545")]
    pub ws_endpoint: String,
    #[clap(long, default_value = "http://localhost:8545")]
    pub http_endpoint: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Subcommand)]
pub enum Command {
    /// Deploy subcommand
    Deploy,
    /// Kitchen sink subcommand
    KitchenSink {
        #[clap(long, default_value = "world")]
        task_message: String,
    },
}
