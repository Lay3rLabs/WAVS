use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Subcommand)]
pub enum Command {
    /// Deploy subcommand
    Deploy(DeployArgs)
}

#[derive(Clone, Args)]
pub struct DeployArgs {
    #[clap(long, default_value = "ws://localhost:8545")]
    pub ws_endpoint: String,
    #[clap(long, default_value = "http://localhost:8545")]
    pub http_endpoint: String,
}