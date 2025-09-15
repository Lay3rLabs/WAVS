mod command;
mod service;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::command::{deploy_service, send_triggers, Command};

#[tokio::main]
async fn main() {
    // setup tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .unwrap();

    let command = Command::parse();

    match command {
        Command::DeployService => {
            deploy_service::run().await;
        }
        Command::SendTriggers { count } => {
            send_triggers::run(count).await;
        }
    }
}
