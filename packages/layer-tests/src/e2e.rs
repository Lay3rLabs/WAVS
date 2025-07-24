mod chain_names;
mod clients;
mod components;
mod config;
mod handles;
mod helpers;
mod matrix;
mod runner;
mod test_definition;
mod test_registry;

use components::ComponentSources;
use config::Configs;
use dashmap::DashMap;
use handles::AppHandles;
pub use matrix::*;
use runner::Runner;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    telemetry::{setup_metrics, setup_tracing, Metrics},
};

use crate::{args::TestArgs, config::TestConfig, e2e::test_registry::CosmosTriggerCodeMap};

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

        let component_sources =
            ComponentSources::new(&configs, &clients.http_client, &clients.aggregator_client).await;

        let cosmos_trigger_code_map = CosmosTriggerCodeMap::new(DashMap::new());

        // Create test registry from test mode
        let registry = test_registry::TestRegistry::from_test_mode(
            mode,
            &configs.chains,
            &clients,
            &cosmos_trigger_code_map,
        )
        .await;

        // Create and run the test runner (services will be deployed just-in-time)
        Runner::new(
            clients,
            registry,
            component_sources,
            cosmos_trigger_code_map,
        )
        .run_tests()
        .await;
    });

    tracing::warn!("*************************************");
    tracing::warn!("All tests completed, shutting down...");
    tracing::warn!("*************************************");

    ctx.kill();
    let join_results = handles.try_join();
    for result in join_results {
        if let Err(e) = result {
            tracing::warn!(
                "error shutting down after tests completed successfully: {:?}",
                e
            );
        }
    }
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
