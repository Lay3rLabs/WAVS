mod add_task;
mod clients;
mod config;
mod digests;
mod handles;
pub mod matrix;
mod runner;
mod services;

use config::Configs;
use digests::Digests;
use handles::AppHandles;
use services::Services;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
};

use crate::{args::TestArgs, config::TestConfig};

pub fn run(args: TestArgs) {
    let config: TestConfig = ConfigBuilder::new(args).build().unwrap();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(config.tracing_env_filter().unwrap())
        .try_init()
        .unwrap();

    let configs: Configs = config.into();

    let ctx = AppContext::new();

    let handles = AppHandles::start(&ctx, &configs);

    let clients = clients::Clients::new(ctx.clone(), &configs);

    let digests = Digests::new(ctx.clone(), &configs, &clients.http_client);

    let services = Services::new(ctx.clone(), &configs, &clients, &digests);

    runner::run_tests(ctx.clone(), configs, clients, services);

    ctx.kill();
    handles.join();
}
