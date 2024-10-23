// no tests in this file, just helpers to assist the actual tests
use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasmatic::{
    args::CliArgs,
    config::{Config, ConfigBuilder},
};

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
}

impl TestApp {
    pub fn default_cli_args() -> CliArgs {
        // get the path relative from this source file, regardless of where we run the test from
        CliArgs {
            home_dir: Some(
                PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .join(ConfigBuilder::DIRNAME),
            ),
            // this purposefully points at a non-existing file
            // so that we don't load a real .env in tests
            dotenv: Some(
                PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .join(ConfigBuilder::DIRNAME)
                    .join("non-existing-testdotenv"),
            ),
            port: None,
            log_level: None,
            host: None,
            data: None,
            cors_allowed_origins: None,
        }
    }

    pub async fn new() -> Self {
        Self::new_with_args(Self::default_cli_args()).await
    }

    pub async fn new_with_args(cli_args: CliArgs) -> Self {
        let config = ConfigBuilder::new(cli_args).build().await.unwrap();

        init(&config).await;

        Self {
            config: Arc::new(config),
        }
    }
}

async fn init(config: &Config) {
    // gate this initialization section to only run one time globally
    {
        static INIT: LazyLock<tokio::sync::Mutex<bool>> =
            LazyLock::new(|| tokio::sync::Mutex::new(false));

        let mut init = INIT.lock().await;

        if !*init {
            *init = true;

            // we want to be able to see tracing info in tests
            // also, although we could technically just store a separate tracing handle in each app
            // this serves as a good sanity check that we're only initializing once
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .without_time()
                        .with_target(false),
                )
                .with(config.tracing_env_filter().unwrap())
                .try_init()
                .unwrap();
        }
    }
}
