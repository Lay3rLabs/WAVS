mod chain_names;
mod clients;
mod components;
mod config;
mod handles;
mod helpers;
mod matrix;
mod test_definition;
mod test_registry;
mod test_runner;
mod types;

use components::ComponentSources;
use config::Configs;
use handles::AppHandles;
pub use matrix::*;
use test_runner::TestRunner;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    telemetry::{setup_metrics, setup_tracing, Metrics},
};

use crate::{args::TestArgs, config::TestConfig};

pub fn run(args: TestArgs, ctx: AppContext) {
    let config: TestConfig = ConfigBuilder::new(args).build().unwrap();
    let mode = config.mode.clone();

    // setup tracing
    let tracer_provider = if let Some(collector) = config.jaeger.clone() {
        Some(ctx.rt.block_on({
            let config = config.clone();
            async move {
                setup_tracing(
                    &collector,
                    "wavs-tests",
                    config.tracing_env_filter().unwrap(),
                )
            }
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

    let meter_provider = if let Some(collector) = config.prometheus.as_ref() {
        Some(
            ctx.rt
                .block_on(async move { setup_metrics(collector, "wavs_test_metrics") }),
        )
    } else {
        None
    };

    let meter = opentelemetry::global::meter("wavs_test_metrics");
    let metrics = Metrics::new(&meter);

    let configs: Configs = config.into();

    let handles = AppHandles::start(&ctx, &configs, metrics);

    ctx.rt.block_on(async {
        let clients = clients::Clients::new(&configs).await;

        let component_sources = ComponentSources::new(&configs, &clients.http_client).await;

        // Create test registry from test mode
        let mut registry =
            test_registry::TestRegistry::from_test_mode(mode, &configs.chains, &clients).await;

        // Deploy services from registry
        tracing::info!("Deploying services for tests...");
        registry.deploy_services(&clients, &component_sources).await;

        // Create and run the test runner
        TestRunner::new(clients, registry).run_tests().await;
    });

    ctx.kill();
    handles.join();
    if let Some(tracer) = tracer_provider {
        tracer
            .shutdown()
            .expect("TracerProvider should shutdown successfully")
    }
    if let Some(meter) = meter_provider {
        meter
            .shutdown()
            .expect("MeterProvider should shutdown successfully")
    }
}
