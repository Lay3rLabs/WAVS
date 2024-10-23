use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasmatic::{args::CliArgs, config::ConfigBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();
    let config = ConfigBuilder::new(args).build().await?;

    // setup tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(config.tracing_env_filter()?)
        .try_init()?;

    tracing::info!("starting wasmatic");
    tracing::info!("{:#?}", config);

    Ok(())
}
