mod add_task;
mod clients;
mod components;
mod config;
mod handles;
mod helpers;
pub mod matrix;
mod test_definition;
mod test_registry;
mod test_runner;
mod types;

use components::ComponentSources;
use config::Configs;
use handles::AppHandles;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    telemetry::{setup_metrics, setup_tracing, Metrics},
};

use crate::{args::TestArgs, config::TestConfig};

pub fn run(args: TestArgs, ctx: AppContext) {
    let isolated = args.isolated.clone();
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

    let clients = clients::Clients::new(ctx.clone(), &configs);

    let component_sources = ComponentSources::new(ctx.clone(), &configs, &clients.http_client);

    // Create test registry from test mode
    let mut registry = test_registry::TestRegistry::from_test_mode(&mode, &configs.chains).unwrap();

    // Deploy services for tests
    match ctx.rt.block_on(async {
        tracing::info!("Deploying services for tests...");
        registry.deploy_services(&clients, &component_sources).await
    }) {
        Ok(_) => {
            // Create and run the test runner
            let test_runner = test_runner::TestRunner::new(ctx.clone(), clients, registry);

            // If a specific test is requested, run just that test
            if let Some(test_name) = &isolated {
                tracing::info!("Running isolated test: {}", test_name);
                match test_runner.run_test_by_name(test_name) {
                    Ok(_) => tracing::info!("Isolated test {} passed", test_name),
                    Err(e) => tracing::error!("Isolated test {} failed: {:?}", test_name, e),
                }
            } else {
                // Otherwise run all tests
                test_runner.run_tests();
            }
        }
        Err(e) => {
            tracing::error!("Failed to deploy services for tests: {:?}", e);
        }
    }

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
