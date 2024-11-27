mod args;
mod config;
mod context;
mod http;

use args::CliArgs;
use config::ConfigBuilder;
use context::AppContext;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn run_server(ctx: AppContext, config: config::Config) {
    ctrlc::set_handler({
        let ctx = ctx.clone();
        move || {
            ctx.kill();
        }
    })
    .unwrap();

    // start the http server in its own thread
    let server_handle = std::thread::spawn({
        let ctx = ctx.clone();
        move || {
            http::server::start(ctx, config).unwrap();
        }
    });

    // wait for all threads to finish

    server_handle.join().unwrap();
}

fn main() {
    let args = CliArgs::parse();
    let config = ConfigBuilder::new(args).build().unwrap();

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
