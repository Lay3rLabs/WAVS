mod command;
mod service;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::command::{deploy_aggregator, deploy_service, send_packet, send_triggers, Command};

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
        Command::DeployAggregator => {
            deploy_aggregator::run().await;
        }
        Command::DeployService { sleep_ms } => {
            deploy_service::run(sleep_ms).await;
        }
        Command::SendPacket => {
            send_packet::run().await;
        }
        Command::SendTriggers {
            count,
            wait_for_completion,
        } => {
            send_triggers::run(count, wait_for_completion).await;
        }
    }
}
