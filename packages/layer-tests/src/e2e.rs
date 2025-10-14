mod chains;
mod clients;
mod components;
mod config;
mod handles;
mod helpers;
mod matrix;
mod report;
mod runner;
mod service_managers;
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
    test_utils::middleware::MiddlewareInstance,
};

use crate::{
    args::TestArgs,
    config::{TestConfig, TestMode},
    e2e::{
        report::TestReport, service_managers::ServiceManagers, test_registry::CosmosTriggerCodeMap,
    },
};

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

    let prometheus_push_interval = 1;
    let meter_provider = if let Some(collector) = config.prometheus.as_ref() {
        Some(ctx.rt.block_on(async move {
            setup_metrics(
                collector,
                "wavs_test_metrics",
                Some(prometheus_push_interval),
            )
        }))
    } else {
        None
    };

    let meter = opentelemetry::global::meter("wavs_test_metrics");
    let metrics = Metrics::new(meter);

    let configs: Configs = config.into();

    let handles = AppHandles::start(&ctx, &configs, metrics, configs.middleware_type);
    tracing::info!("Background processes started");

    let mut kill_receiver = ctx.get_kill_receiver();

    ctx.rt.block_on(async {
        tokio::select! {
            _ = kill_receiver.recv() => {
                tracing::debug!("Test runner killed");
            },
            _ = _run(configs, mode, handles.middleware_instance.clone()) => {
                tracing::debug!("Test runner completed");
            }
        }
    });

    let was_killed = ctx.killed();

    tracing::warn!("shutting down...");

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

    if was_killed {
        panic!("Test runner was killed, exiting with error");
    }
}

async fn _run(configs: Configs, mode: TestMode, middleware_instance: MiddlewareInstance) {
    let report = TestReport::new();

    let clients = clients::Clients::new(&configs).await;

    let cosmos_trigger_code_map = CosmosTriggerCodeMap::new(DashMap::new());

    // Create test registry from test mode
    let registry = test_registry::TestRegistry::from_test_mode(
        mode,
        configs.chains.clone(),
        &clients,
        &cosmos_trigger_code_map,
    )
    .await;

    // bootstrap service managers
    let mut service_managers = ServiceManagers::new(configs.clone());
    service_managers
        .bootstrap(&registry, &clients, middleware_instance)
        .await;

    // upload components
    let component_sources = ComponentSources::new(
        &configs,
        &registry,
        &clients.http_client,
        &clients.aggregator_clients,
        &configs.aggregators,
    )
    .await;

    // create the real services (deploy contracts etc.)

    let services = service_managers
        .create_real_wavs_services(
            &registry,
            &clients,
            &component_sources,
            cosmos_trigger_code_map.clone(),
        )
        .await;

    // Create and run the test runner (services will be deployed just-in-time)
    Runner::new(
        clients,
        registry,
        component_sources,
        service_managers,
        cosmos_trigger_code_map,
        report.clone(),
    )
    .run_tests(services)
    .await;

    report.print();
}
