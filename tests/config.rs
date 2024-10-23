mod helpers;
use helpers::TestApp;
use std::{path::PathBuf, sync::LazyLock};
use wasmatic::{args::CliArgs, config::ConfigBuilder};

// this test is confiming the user overrides for filepath work as expected
// but it does not test the complete list of fallbacks past those first few common cases
// because the complete list will change depending on the platform, global env vars, etc.
#[tokio::test]
async fn config_filepath() {
    fn filepaths(home_dir: Option<PathBuf>) -> Vec<PathBuf> {
        ConfigBuilder::new(CliArgs {
            home_dir,
            dotenv: None,
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

// tests that we can override file settings with env vars
// and that env filter with comma-delimited values and spaces works
#[tokio::test]
async fn override_with_env_var() {
    static TRACING_FILTER: LazyLock<tracing_subscriber::EnvFilter> = LazyLock::new(|| {
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("debug".parse().unwrap())
            .add_directive("foo=trace".parse().unwrap())
    });

    let config = TestApp::new().await.config;

    // sanity check that our made-up filter is not the same as the real one
    assert_ne!(
        config.build_tracing_filter().unwrap().to_string(),
        TRACING_FILTER.to_string()
    );

    // replace the var and check that it is now what we expect
    // needs to be in an async function
    {
        temp_env::async_with_vars(
            [(
                format!("{}_{}", ConfigBuilder::ENV_VAR_PREFIX, "TRACING_FILTER"),
                Some("debug, foo=trace"),
            )],
            check(),
        )
        .await;

        async fn check() {
            let config = TestApp::new().await.config;
            assert_eq!(
                config.build_tracing_filter().unwrap().to_string(),
                TRACING_FILTER.to_string()
            );
        }
    }
}

// tests that we load a dotenv file correctly, if specified in cli args
#[tokio::test]
async fn loads_dotenv() {
    // careful! once we load the dotenv file, that's it, other tests may see it
    let _ = TestApp::new_with_dotenv().await;

    // if we try to check against meaningful env vars, we may conflict with user settings
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

// tests that we can override defaults with config-file vars
#[tokio::test]
async fn file_default() {
    let config = TestApp::new().await.config;
    assert_eq!(
        config.tracing_filter,
        ["info", "wasmatic=debug", "just_to_confirm_test=debug"]
    );
}
