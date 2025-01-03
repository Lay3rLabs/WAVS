use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aggregator::{args::CliArgs, config::Config, run_server, AppContext};
use utils::config::{ConfigBuilder, ConfigExt};

fn main() {
    let args = CliArgs::parse();
    let config: Config = ConfigBuilder::new(args).build().unwrap();

    // setup tracing
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

    run_server(ctx, config);

}
