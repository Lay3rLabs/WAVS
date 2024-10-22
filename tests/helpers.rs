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

static INIT: LazyLock<tokio::sync::Mutex<bool>> = LazyLock::new(|| tokio::sync::Mutex::new(false));

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
}

impl TestApp {
    pub async fn new() -> Self {
        // get the path relative from this source file, regardless of where we run the test from
        let cli_args = CliArgs {
            home_dir: Some(
                PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .join(ConfigBuilder::DIRNAME),
            ),
            dotenv: None,
        };
        let config = ConfigBuilder::new(cli_args).unwrap().build().await.unwrap();

        // gate the initialization to only run one time
        let mut init = INIT.lock().await;
        if !*init {
            *init = true;

            // we want to be able to see tracing info in tests
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .without_time()
                        .with_target(false),
                )
                .with(config.build_tracing_filter().unwrap())
                .try_init()
                .unwrap();
        }

        Self {
            config: Arc::new(config),
        }
    }
}
