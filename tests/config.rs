mod helpers;
use helpers::TestApp;
use std::{path::PathBuf, sync::LazyLock};
use wasmatic::{args::CliArgs, config::ConfigBuilder};

#[tokio::test]
// this test is essentially confiming the user overrides work as expected
// there are additional fallback directories to try in the list depending on the platform, global env vars, etc.
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

#[tokio::test]
// tests that we can override file settings with env vars
// and that env filter with comma-delimited values and spaces works
async fn override_with_env_var() {
    static TRACING_FILTER: LazyLock<tracing_subscriber::EnvFilter> = LazyLock::new(|| {
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("debug".parse().unwrap())
            .add_directive("foo=trace".parse().unwrap())
    });

    let config = TestApp::new().await.config;

    // sanity check
    assert_ne!(
        config.build_tracing_filter().unwrap().to_string(),
        TRACING_FILTER.to_string()
    );

    async fn check() {
        let config = TestApp::new().await.config;
        assert_eq!(
            config.build_tracing_filter().unwrap().to_string(),
            TRACING_FILTER.to_string()
        );
    }

    temp_env::async_with_vars(
        [(
            format!("{}_{}", ConfigBuilder::ENV_VAR_PREFIX, "TRACING_FILTER"),
            Some("debug, foo=trace"),
        )],
        check(),
    )
    .await;
}

#[tokio::test]
#[ignore]
// tests that we load a dotenv file correctly
async fn loads_dotenv() {
    let config = TestApp::new_no_dotenv().await.config;
    assert_eq!(config.port, 8000);

    let config = TestApp::new().await.config;
    assert_eq!(config.port, 1234567);
}

#[tokio::test]
// just tests that we can override defaults with file settings
async fn file_default() {
    let config = TestApp::new().await.config;
    assert_eq!(
        config.tracing_filter,
        ["info", "wasmatic=debug", "just_to_confirm_test=debug"]
    );
}
