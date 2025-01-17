mod clients;
mod config;
mod cosmos;
mod digests;
mod eth;
mod handles;
pub mod matrix;
mod runner;
mod services;

use config::Configs;
use digests::Digests;
use handles::AppHandles;
use services::Services;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{config::ConfigExt, context::AppContext};

use crate::{args::TestArgs, config::TestConfig};

pub fn run(args: TestArgs) {
    let config = TestConfig::new(args);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(config.tracing_env_filter().unwrap())
        .try_init()
        .unwrap();

    let ctx = AppContext::new();

    let (mut eth_chains, mut cosmos_chains) = (
        eth::start_chains(&config),
        cosmos::start_chains(ctx.clone(), &config),
    );

    let configs = Configs::new(
        config,
        eth_chains
            .iter()
            .map(|(chain_config, _)| chain_config.clone())
            .collect(),
        cosmos_chains
            .iter()
            .map(|(chain_config, _)| chain_config.clone())
            .collect(),
    );

    let handles = AppHandles::start(
        &ctx,
        &configs,
        eth_chains.drain(..).map(|(_, handle)| handle).collect(),
        cosmos_chains.drain(..).map(|(_, handle)| handle).collect(),
    );

    let clients = clients::Clients::new(ctx.clone(), &configs);

    let digests = Digests::new(ctx.clone(), &configs, &clients.http_client);

    let services = Services::new(ctx.clone(), &configs, &clients, &digests);

    runner::run_tests(ctx.clone(), clients, services);

    ctx.kill();
    handles.join();
}
