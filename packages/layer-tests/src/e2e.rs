mod add_task;
mod clients;
mod components;
mod config;
mod handles;
pub mod matrix;
mod runner;
mod services;

use components::ComponentSources;
use config::Configs;
use handles::AppHandles;
use services::Services;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
};
use wavs::telemetry::setup_tracing;

use crate::{args::TestArgs, config::TestConfig};

pub fn run(args: TestArgs, ctx: AppContext) {
    let config: TestConfig = ConfigBuilder::new(args).build().unwrap();

    // setup tracing
    let tracer_provider = if let Some(collector) = config.jaeger.clone() {
        let config = config.clone();
        Some(ctx.rt.spawn_blocking(move || {
            let config = config.clone();
            async move { setup_tracing(&collector, config.tracing_env_filter().unwrap()) }
        }))
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_target(false),
            )
            .with(config.tracing_env_filter().unwrap())
            .try_init()
            .unwrap();
        None
    };

    let configs: Configs = config.into();

    let handles = AppHandles::start(&ctx, &configs);

    let clients = clients::Clients::new(ctx.clone(), &configs);

    let component_sources = ComponentSources::new(ctx.clone(), &configs, &clients.http_client);

    let services = Services::new(ctx.clone(), &configs, &clients, &component_sources);

    runner::run_tests(ctx.clone(), configs, clients, services);

    ctx.kill();
    handles.join();
    if let Some(tracer) = tracer_provider {
        ctx.rt.block_on(async move {
            tracer
                .await
                .unwrap()
                .await
                .shutdown()
                .expect("TracerProvider should shutdown successfully")
        })
    };
}
