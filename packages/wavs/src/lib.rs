pub mod apis;
pub mod args;
pub mod config;
pub mod dispatcher; // where we have the high-level dispatcher
pub mod engine; // where we manage and execute wasm
pub mod http;
pub mod submission; // where we submit the results to the chain
pub mod test_utils;
pub mod triggers; // where we handle the trigger runtime

use apis::dispatcher::DispatchManager;
use config::Config;

// This section is called from both main and end-to-end tests
use dispatcher::CoreDispatcher;
use std::sync::Arc;
use utils::context::AppContext;

/// Entry point to start up the whole server
/// Called from main and end-to-end tests
pub fn run_server(ctx: AppContext, config: Config, dispatcher: Arc<CoreDispatcher>) {
    let _ = ctrlc::set_handler({
        let ctx = ctx.clone();
        move || {
            ctx.kill();
        }
    });

    // start the http server in its own thread
    let server_handle = std::thread::spawn({
        let dispatcher = dispatcher.clone();
        let ctx = ctx.clone();
        move || {
            http::server::start(ctx, config, dispatcher).unwrap();
        }
    });

    let dispatcher_handle = std::thread::spawn(move || {
        dispatcher.start(ctx).unwrap();
    });

    // wait for all threads to finish

    server_handle.join().unwrap();
    dispatcher_handle.join().unwrap();
}

// the test version of init_tracing does not take a config
// since config itself is tested and modified from different parallel tests
// therefore, this only uses the default tracing settings
// it's not gated out because it is used in benches and integration tests as well
pub fn init_tracing_tests() {
    use std::sync::LazyLock;

    // however, it has an extra complexity of race conditions across threads
    // so we use a Mutex to ensure we only initialize once globally
    static INIT: LazyLock<std::sync::Mutex<bool>> = LazyLock::new(|| std::sync::Mutex::new(false));

    let mut init = INIT.lock().unwrap();

    if !*init {
        *init = true;

        // we want to be able to see tracing info in tests
        // also, although we could technically just store a separate tracing handle in each app
        // this serves as a good sanity check that we're only initializing once
        tracing_subscriber::fmt::init();
        tracing::debug!("Tracing initialized for tests");
    }
}

pub mod telemetry {
    use opentelemetry::{global, trace::TracerProvider as _};
    use opentelemetry_otlp::{SpanExporter, WithExportConfig};
    use opentelemetry_sdk::{
        resource::Resource,
        trace::{self, Sampler, SdkTracerProvider},
    };
    use tracing_subscriber::layer::SubscriberExt;

    pub fn setup_tracing(
        collector: &str,
        tracer: &str,
        filters: tracing_subscriber::EnvFilter,
    ) -> SdkTracerProvider {
        global::set_text_map_propagator(opentelemetry_jaeger_propagator::Propagator::new());
        let endpoint = format!("{}/v1/traces", collector);
        let exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("Failed to build OTLP exporter");

        let batch_processor = trace::BatchSpanProcessor::builder(exporter).build();

        let provider = SdkTracerProvider::builder()
            .with_span_processor(batch_processor)
            .with_sampler(Sampler::AlwaysOn)
            .with_resource(Resource::builder().with_service_name("wavs").build())
            .build();
        global::set_tracer_provider(provider.clone());
        let tracer = provider.tracer(tracer.to_owned());
        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

        let subscriber = tracing_subscriber::Registry::default()
            .with(filters)
            .with(tracing_subscriber::fmt::layer()) // console logging layer
            .with(telemetry);

        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set global default subscriber");

        tracing::info!("Jaeger tracing enabled");
        provider
    }
}
