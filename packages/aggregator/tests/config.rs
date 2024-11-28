use aggregator::test_utils::app::TestApp;
use alloy::primitives::Address;
use temp_env::async_with_vars;

use aggregator::{
    args::CliArgs,
    config::{Config, ConfigBuilder},
};
use std::{path::PathBuf, sync::LazyLock};

// this test is confiming the user overrides for filepath work as expected
// but it does not test the complete list of fallbacks past those first few common cases
// because the complete list will change depending on the platform, global env vars, etc.
#[tokio::test]
async fn config_filepath() {
    let filepaths = |home: Option<PathBuf>| -> Vec<PathBuf> {
        let config_builder = ConfigBuilder::new(CliArgs {
            home,
            dotenv: None,
            ..TestApp::default_cli_args()
        });

        let cli_env_args = config_builder.merge_cli_env_args().unwrap();

        ConfigBuilder::filepaths_to_try(&cli_env_args)
    };

    // make sure all the test directories are not there by default
    let default_dirs = filepaths(None);
    for i in 1..=10 {
        assert!(!default_dirs
            .contains(&PathBuf::from(format!("/tmp{}", i)).join(ConfigBuilder::FILENAME)));
    }

    // if provide a specific home directory, then it is the first one to try
    assert_eq!(
        filepaths(Some("/tmp1".into())).first().unwrap(),
        &PathBuf::from("/tmp1").join(ConfigBuilder::FILENAME)
    );

    // even if we also provide it in an env var, it still takes precedence
    temp_env::with_vars(
        [(
            format!("{}_{}", CliArgs::ENV_VAR_PREFIX, "HOME"),
            Some("/tmp2"),
        )],
        || {
            assert_eq!(
                filepaths(Some("/tmp1".into())).first().unwrap(),
                &PathBuf::from("/tmp1").join(ConfigBuilder::FILENAME)
            );
        },
    );

    // but if we provide an env var, and not a specific home directory, then env var becomes the first
    temp_env::with_vars(
        [(
            format!("{}_{}", CliArgs::ENV_VAR_PREFIX, "HOME"),
            Some("/tmp2"),
        )],
        || {
            assert_eq!(
                filepaths(None).first().unwrap(),
                &PathBuf::from("/tmp2").join(ConfigBuilder::FILENAME),
            );
        },
    );
}

// tests that default values are set correctly
#[tokio::test]
async fn config_default() {
    // port is *not* set in the test toml file
    assert_eq!(TestApp::new().await.config.port, Config::default().port);
}

// tests that we can configure array-strings, and it overrides as expected
#[tokio::test]
async fn config_array_string() {
    static TRACING_ENV_FILTER_ENV: LazyLock<tracing_subscriber::EnvFilter> = LazyLock::new(|| {
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("debug".parse().unwrap())
            .add_directive("foo=trace".parse().unwrap())
    });
    static TRACING_ENV_FILTER_CLI: LazyLock<tracing_subscriber::EnvFilter> = LazyLock::new(|| {
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("trace".parse().unwrap())
            .add_directive("bar=debug".parse().unwrap())
    });

    // it's set in the file too for other tests, but here we need to be explicit
    let get_config = || async { TestApp::new().await.config };
    let config = async_with_vars(
        [(
            format!("{}_{}", CliArgs::ENV_VAR_PREFIX, "LOG_LEVEL"),
            Some("info, aggregator=debug, just_to_confirm_test=debug"),
        )],
        get_config(),
    )
    .await;

    assert_eq!(
        config.log_level,
        ["info", "aggregator=debug", "just_to_confirm_test=debug"]
    );

    // replace the var and check that it is now what we expect
    // env replacement needs to be in an async function
    {
        let check = || async {
            // first - if we don't set a CLI var, it should use the env var
            let config = TestApp::new().await.config;
            assert_eq!(
                config.tracing_env_filter().unwrap().to_string(),
                TRACING_ENV_FILTER_ENV.to_string()
            );

            // but then, even when the env var is available, if we set a CLI var, it should override
            let mut cli_args = TestApp::default_cli_args();
            cli_args.log_level = TRACING_ENV_FILTER_CLI
                .to_string()
                .split(",")
                .map(|s| s.to_string())
                .collect();

            let config = TestApp::new_with_args(cli_args).await.config;

            assert_eq!(
                config.tracing_env_filter().unwrap().to_string(),
                TRACING_ENV_FILTER_CLI.to_string()
            );
        };

        temp_env::async_with_vars(
            [(
                format!("{}_{}", CliArgs::ENV_VAR_PREFIX, "LOG_LEVEL"),
                Some("debug, foo=trace"),
            )],
            check(),
        )
        .await;
    }
}

// tests that we load a dotenv file correctly, if specified in cli args
#[tokio::test]
async fn config_dotenv() {
    let mut cli_args = TestApp::default_cli_args();

    cli_args.dotenv = Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(ConfigBuilder::DIRNAME)
            .join("testdotenv"),
    );

    let _ = TestApp::new_with_args(cli_args).await;

    // if we try to check against meaningful env vars, we may conflict with other tests and/or user settings
    // so just check for a dummy value since this test only cares about the dotenv file itself
    // coverage of environment var overrides is in other tests with temp_env scopes
    assert_eq!(
        std::env::var(format!("{}_RANDOM_TEST_VALUE", CliArgs::ENV_VAR_PREFIX)).unwrap(),
        "hello world"
    );

    // unset the value, just to play nice, though this could be a race condition (see docs on remove_var)
    std::env::remove_var(format!("{}_RANDOM_TEST_VALUE", CliArgs::ENV_VAR_PREFIX))
}

// tests that we load chain config section correctly
#[tokio::test]
async fn config_mnemonic() {
    let config = TestApp::new().await.config;

    let signer = config.signer().unwrap();
    assert_eq!(
        signer.address(),
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse::<Address>()
            .unwrap()
    );
    // Set up by the toml config file
    assert_eq!(config.chain, "anvil");

    // change the mnemonic via cli
    let mut cli_args = TestApp::default_cli_args();
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_owned();
    cli_args.mnemonic = Some(mnemonic);
    let config = TestApp::new_with_args(cli_args).await.config;
    let signer2 = config.signer().unwrap();
    assert_eq!(
        signer2.address(),
        "0x9858effd232b4033e47d90003d41ec34ecaeda94"
            .parse::<Address>()
            .unwrap()
    );

    // change the endpoint and chain
    let mut cli_args = TestApp::default_cli_args();
    cli_args.endpoint = Some("ws://localhost:1234".to_owned());
    cli_args.chain = Some("notanvil".to_owned());
    let config = TestApp::new_with_args(cli_args).await.config;
    assert_eq!(config.endpoint, "ws://localhost:1234");
    assert_eq!(config.chain, "notanvil");
}
