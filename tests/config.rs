mod helpers;
use helpers::TestApp;
use std::{path::PathBuf, sync::LazyLock};
use wasmatic::{
    args::CliArgs,
    config::{Config, ConfigBuilder},
};

// this test is confiming the user overrides for filepath work as expected
// but it does not test the complete list of fallbacks past those first few common cases
// because the complete list will change depending on the platform, global env vars, etc.
#[tokio::test]
async fn config_filepath() {
    fn filepaths(home_dir: Option<PathBuf>) -> Vec<PathBuf> {
        ConfigBuilder::new(CliArgs {
            home_dir,
            dotenv: None,
            ..TestApp::default_cli_args()
        })
        .filepaths_to_try()
    }

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

    // if we provide a specific home directory, and env var, then they are tried in that order
    temp_env::with_vars(
        [(
            format!("{}_{}", ConfigBuilder::ENV_VAR_PREFIX, "HOME"),
            Some("/tmp2"),
        )],
        || {
            assert_eq!(
                filepaths(Some("/tmp1".into()))
                    .into_iter()
                    .take(2)
                    .collect::<Vec<PathBuf>>(),
                vec![
                    PathBuf::from("/tmp1").join(ConfigBuilder::FILENAME),
                    PathBuf::from("/tmp2").join(ConfigBuilder::FILENAME),
                ]
            );
        },
    );

    // if we provide an env var, but not a specific home directory, then env var becomes the first
    temp_env::with_vars(
        [(
            format!("{}_{}", ConfigBuilder::ENV_VAR_PREFIX, "HOME"),
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

    let config = TestApp::new().await.config;

    // sanity check that our log_level was correctly loaded from file
    assert_eq!(
        config.log_level,
        ["info", "wasmatic=debug", "just_to_confirm_test=debug"]
    );

    // replace the var and check that it is now what we expect
    // needs to be in an async function
    {
        temp_env::async_with_vars(
            [(
                format!("{}_{}", ConfigBuilder::ENV_VAR_PREFIX, "LOG_LEVEL"),
                Some("debug, foo=trace"),
            )],
            check(),
        )
        .await;

        async fn check() {
            let config = TestApp::new().await.config;
            assert_eq!(
                config.tracing_env_filter().unwrap().to_string(),
                TRACING_ENV_FILTER_ENV.to_string()
            );

            let mut cli_args = TestApp::default_cli_args();
            cli_args.log_level = TRACING_ENV_FILTER_CLI.to_string().split(",").map(|s| s.to_string()).collect();

            let config = TestApp::new_with_args(cli_args).await.config;

            assert_eq!(
                config.tracing_env_filter().unwrap().to_string(),
                TRACING_ENV_FILTER_CLI.to_string()
            );
        }
    }
}

// tests that we load a dotenv file correctly, if specified in cli args
#[tokio::test]
async fn config_dotenv() {
    let mut cli_args = TestApp::default_cli_args();
    // this points to a real file
    cli_args.dotenv = Some(
        PathBuf::from(file!())
            .parent()
            .unwrap()
            .join(ConfigBuilder::DIRNAME)
            .join("testdotenv"),
    );

    let _ = TestApp::new_with_args(cli_args).await;

    // if we try to check against meaningful env vars, we may conflict with other tests and/or user settings
    // so just check for a dummy value since this test only cares about the dotenv file itself
    // coverage of environment var overrides is in other tests with temp_env scopes
    assert_eq!(
        std::env::var(format!(
            "{}_RANDOM_TEST_VALUE",
            ConfigBuilder::ENV_VAR_PREFIX
        ))
        .unwrap(),
        "hello world"
    );

    // unset the value, just to play nice, though this could be a race condition (see docs on remove_var)
    std::env::remove_var(format!(
        "{}_RANDOM_TEST_VALUE",
        ConfigBuilder::ENV_VAR_PREFIX
    ))
}
