use anyhow::Result;
use clap::Parser;
use std::process::exit;
use tracing_subscriber::EnvFilter;
use wasmatic::commands::ResetCommand;
use wasmatic::commands::UpCommand;

fn version() -> &'static str {
    option_env!("CARGO_VERSION_INFO").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Wasmatic CLI.
#[derive(Parser)]
#[clap(
    bin_name = "wasmatic",
    version,
    propagate_version = true,
    arg_required_else_help = true
)]
#[command(version = version())]
enum WasmaticCli {
    Up(UpCommand),
    Reset(ResetCommand),
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if let Err(e) = match WasmaticCli::parse() {
        WasmaticCli::Up(cmd) => cmd.exec().await,
        WasmaticCli::Reset(cmd) => cmd.exec().await,
    } {
        eprintln!("error: {e:?}");
        exit(1);
    }

    Ok(())
}
