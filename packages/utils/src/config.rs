use anyhow::{bail, Context, Result};
use figment::{providers::Format, Figment};
use layer_climb::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::BTreeMap, marker::PhantomData, path::PathBuf};

use crate::{
    error::ChainConfigError,
    eth_client::{EthClientConfig, EthClientTransport},
};
use wavs_types::ChainName;

/// The builder we use to build Config
#[derive(Debug)]
pub struct ConfigBuilder<CONFIG, ARG> {
    pub cli_env_args: ARG,
    _config: PhantomData<CONFIG>,
}

pub trait CliEnvExt: Serialize + DeserializeOwned + Default + std::fmt::Debug {
    // e.g. "WAVS"
    const ENV_VAR_PREFIX: &'static str;

    // an optional argument to specify the home directory
    // if not supplied, config will try a series of fallbacks
    fn home_dir(&self) -> Option<PathBuf>;

    // an optional argument to specify the home directory
    // if not supplied, config will try a series of fallbacks
    fn dotenv_path(&self) -> Option<PathBuf>;

    fn merge_cli_env_args(&self) -> Result<Self> {
        let env_prefix = format!("{}_", Self::ENV_VAR_PREFIX);

        let _self = Figment::new()
            .merge(figment::providers::Env::prefixed(&env_prefix))
            .merge(figment::providers::Serialized::defaults(self))
            .extract()?;

        Ok(_self)
    }

    fn env_var(name: &str) -> Option<String> {
        std::env::var(format!("{}_{name}", Self::ENV_VAR_PREFIX)).ok()
    }
}

pub trait ConfigExt: Serialize + DeserializeOwned + Default + std::fmt::Debug {
    // e.g. "wavs.toml"
    const FILENAME: &'static str;

    // the data directory, which is the root of the data storage
    fn with_data_dir(&mut self, f: fn(&mut PathBuf));

    fn log_levels(&self) -> impl Iterator<Item = &str>;

    fn tracing_env_filter(&self) -> Result<tracing_subscriber::EnvFilter> {
        let mut filter = tracing_subscriber::EnvFilter::from_default_env();
        for directive in self.log_levels() {
            match directive.parse() {
                Ok(directive) => filter = filter.add_directive(directive),
                Err(err) => bail!("{}: {}", err, directive),
            }
        }

        Ok(filter)
    }
}

impl<CONFIG: ConfigExt, ARG: CliEnvExt> ConfigBuilder<CONFIG, ARG> {
    pub fn new(cli_env_args: ARG) -> Self {
        Self {
            cli_env_args,
            _config: PhantomData,
        }
    }

    pub fn build(self) -> Result<CONFIG> {
        // try to load dotenv first, since it may affect env vars for filepaths
        let mut dotenv_paths = Vec::new();

        if let Some(dotenv_path) = self.cli_env_args.dotenv_path() {
            dotenv_paths.push(dotenv_path);
        }

        if let Ok(dotenv_path) = std::env::var("WAVS_DOTENV") {
            dotenv_paths.push(PathBuf::from(dotenv_path));
        }

        dotenv_paths.push(std::env::current_dir()?.join(".env"));

        for dotenv_path in dotenv_paths {
            if dotenv_path.exists() {
                if let Err(e) = dotenvy::from_path(dotenv_path) {
                    bail!("Error loading dotenv file: {}", e);
                }
            }
        }

        // first merge the cli and env vars
        let cli_env_args = self.cli_env_args.merge_cli_env_args()?;

        // then get the filepath for our file-based config
        let filepath = ConfigFilePath::new(CONFIG::FILENAME, cli_env_args.home_dir())
            .into_path()
            .context(format!(
                "Error getting config file path (filename: {}, homedir: {:?})",
                CONFIG::FILENAME,
                cli_env_args.home_dir()
            ))?;

        // lastly, our final config, which can have more complex types with easier TOML-like syntax
        // but is overriden by the cli/env args if they exist
        // and also fills in defaults for required values at the end
        let mut config: CONFIG = Figment::new()
            .merge(figment::providers::Toml::file(filepath))
            .merge(figment::providers::Serialized::defaults(cli_env_args))
            .join(figment::providers::Serialized::defaults(CONFIG::default()))
            .extract()?;

        config.with_data_dir(|data_dir| {
            *data_dir = shellexpand::tilde(&data_dir.to_string_lossy())
                .to_string()
                .into();
        });

        Ok(config)
    }
}

// a helper to try a series of fallback paths, looking for a config file
#[derive(Clone, Debug)]
pub struct ConfigFilePath {
    // the filename to look for in each directory, e.g. "wavs.toml"
    pub filename: String,
    // the optional directory set via direct args or env
    pub arg_env_dir: Option<PathBuf>,
}

impl ConfigFilePath {
    pub fn new(filename: impl ToString, arg_env_dir: Option<PathBuf>) -> Self {
        Self {
            filename: filename.to_string(),
            arg_env_dir,
        }
    }

    pub fn into_path(self) -> Option<PathBuf> {
        self.into_possible().into_iter().find(|path| path.exists())
    }

    // tries a series of fallbacks
    pub fn into_possible(self) -> Vec<PathBuf> {
        let Self {
            filename,
            arg_env_dir,
        } = self;

        const DIRNAME: &str = "wavs";

        // the paths returned will be tried in order of pushing
        let mut dirs = Vec::new();

        // explicit, e.g. passing --home /foo to a binary, or env var {ENV_PREFIX}_HOME="/foo"
        // i.e. the path in this case will be /foo/{filename}
        if let Some(dir) = arg_env_dir {
            dirs.push(dir);
        }

        // literal env var WAVS_HOME
        if let Ok(dir) = std::env::var("WAVS_HOME") {
            dirs.push(dir.into());
        }

        // next, check the current working directory, wherever the command is run from
        // i.e. ./{filename}
        if let Ok(dir) = std::env::current_dir() {
            dirs.push(dir);
        }

        // here we want to check the user's home directory directly, not in the `.config` subdirectory
        // in this case, to not pollute the home directory, it looks for ~/.{dirname}/{filename} (e.g. ~/.wavs/wavs.toml)
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(format!(".{}", DIRNAME))) {
            dirs.push(dir);
        }

        // checks the `wavs/wavs.toml` file in the system config directory
        // this will vary, but the final path with then be something like:
        // Linux: ~/.config/wavs/wavs.toml
        // macOS: ~/Library/Application Support/wavs/wavs.toml
        // Windows: C:\Users\MyUserName\AppData\Roaming\wavs\wavs.toml
        if let Some(dir) = dirs::config_dir().map(|dir| dir.join(DIRNAME)) {
            dirs.push(dir);
        }

        // On linux, this may already be added via config_dir above
        // but on macOS and windows, and maybe unix-like environments (msys, wsl, etc)
        // it's helpful to add it explicitly
        // the final path here typically becomes something like ~/.config/wavs/wavs.toml
        if let Some(dir) = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .map(|dir| dir.join(DIRNAME))
        {
            dirs.push(dir);
        }

        // Similarly, `config_dir` above may have already added this
        // but on systems like Windows, it's helpful to add it explicitly
        // since the system may place the config dir in AppData/Roaming
        // but we want to check the user's home dir first
        // this will definitively become something like ~/.config/wavs/wavs.toml
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(".config").join(DIRNAME)) {
            dirs.push(dir);
        }

        // Lastly, try /etc/wavs/wavs.toml
        dirs.push(PathBuf::from("/etc").join(DIRNAME));

        // now we have a list of directories to check, we need to add the filename to each
        let mut all_files: Vec<PathBuf> = dirs.into_iter().map(|dir| dir.join(&filename)).collect();

        all_files.dedup();

        all_files
    }
}

// TODO - impl a custom Deserialize that ensures at *load-time* that keys are unique
// currently we only get that guarantee when we call `get_chain()`
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct ChainConfigs {
    /// Cosmos-style chains (including Layer-SDK)
    pub cosmos: BTreeMap<ChainName, CosmosChainConfig>,
    /// Ethereum-style chains
    pub eth: BTreeMap<ChainName, EthereumChainConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AnyChainConfig {
    Cosmos(CosmosChainConfig),
    Eth(EthereumChainConfig),
}

impl From<ChainConfigs> for BTreeMap<ChainName, AnyChainConfig> {
    fn from(configs: ChainConfigs) -> Self {
        let mut map = BTreeMap::new();
        for (name, config) in configs.cosmos {
            map.insert(name, AnyChainConfig::Cosmos(config));
        }
        for (name, config) in configs.eth {
            map.insert(name, AnyChainConfig::Eth(config));
        }
        map
    }
}

// Cosmos From/Into impls
impl TryFrom<AnyChainConfig> for CosmosChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> std::result::Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Cosmos(config) => Ok(config),
            AnyChainConfig::Eth(_) => Err(ChainConfigError::ExpectedCosmosChain),
        }
    }
}

impl From<CosmosChainConfig> for AnyChainConfig {
    fn from(config: CosmosChainConfig) -> Self {
        AnyChainConfig::Cosmos(config)
    }
}

impl TryFrom<AnyChainConfig> for layer_climb::prelude::ChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> std::result::Result<Self, Self::Error> {
        CosmosChainConfig::try_from(config).map(Into::into)
    }
}

impl TryFrom<layer_climb::prelude::ChainConfig> for AnyChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: layer_climb::prelude::ChainConfig) -> Result<Self, Self::Error> {
        Ok(CosmosChainConfig::try_from(config)?.into())
    }
}

// Ethereum From/Into impls
impl TryFrom<AnyChainConfig> for EthereumChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> std::result::Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Eth(config) => Ok(config),
            AnyChainConfig::Cosmos(_) => Err(ChainConfigError::ExpectedEthChain),
        }
    }
}

impl From<EthereumChainConfig> for AnyChainConfig {
    fn from(config: EthereumChainConfig) -> Self {
        AnyChainConfig::Eth(config)
    }
}

impl ChainConfigs {
    pub fn get_chain(&self, chain_name: &ChainName) -> Result<Option<AnyChainConfig>> {
        match (self.eth.get(chain_name), self.cosmos.get(chain_name)) {
            (Some(_), Some(_)) => {
                Err(ChainConfigError::DuplicateChainName(chain_name.clone()).into())
            }
            (Some(eth), None) => Ok(Some(AnyChainConfig::Eth(eth.clone()))),
            (None, Some(cosmos)) => Ok(Some(AnyChainConfig::Cosmos(cosmos.clone()))),
            (None, None) => Ok(None),
        }
    }

    pub fn all_chain_names(&self) -> Vec<ChainName> {
        self.eth.keys().chain(self.cosmos.keys()).cloned().collect()
    }
}

/// Cosmos chain config with extra info like faucet and mnemonic
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CosmosChainConfig {
    pub chain_id: String,
    pub bech32_prefix: String,
    pub rpc_endpoint: Option<String>,
    pub grpc_endpoint: Option<String>,
    pub gas_price: f32,
    pub gas_denom: String,
    pub faucet_endpoint: Option<String>,
}

/// Ethereum chain config with extra info like faucet and mnemonic
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EthereumChainConfig {
    pub chain_id: String,
    pub ws_endpoint: Option<String>,
    pub http_endpoint: Option<String>,
    pub aggregator_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
}

impl EthereumChainConfig {
    pub fn to_client_config(
        &self,
        hd_index: Option<u32>,
        mnemonic: Option<String>,
        transport: Option<EthClientTransport>,
    ) -> EthClientConfig {
        EthClientConfig {
            ws_endpoint: self.ws_endpoint.clone(),
            http_endpoint: self.http_endpoint.clone(),
            transport,
            hd_index,
            mnemonic,
            gas_estimate_multiplier: None,
        }
    }
}

impl From<CosmosChainConfig> for ChainConfig {
    fn from(config: CosmosChainConfig) -> Self {
        Self {
            chain_id: ChainId::new(config.chain_id),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: None,
            gas_price: config.gas_price,
            gas_denom: config.gas_denom,
            address_kind: AddrKind::Cosmos {
                prefix: config.bech32_prefix,
            },
        }
    }
}

impl TryFrom<ChainConfig> for CosmosChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: ChainConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: config.chain_id.to_string(),
            bech32_prefix: match config.address_kind {
                AddrKind::Cosmos { prefix } => prefix,
                _ => return Err(ChainConfigError::ExpectedCosmosChain),
            },
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            gas_price: config.gas_price,
            gas_denom: config.gas_denom,
            faucet_endpoint: None,
        })
    }
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, sync::LazyLock};

    use serde::{Deserialize, Serialize};

    use crate::{config::ConfigFilePath, serde::deserialize_vec_string};

    use super::{
        ChainConfigs, CliEnvExt, ConfigBuilder, ConfigExt, CosmosChainConfig, EthereumChainConfig,
    };

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestConfig {
        pub data: PathBuf,
        pub port: u16,
        pub log_level: Vec<String>,
    }

    impl Default for TestConfig {
        fn default() -> Self {
            Self {
                data: PathBuf::from("/var/wavs"),
                port: 8000,
                log_level: vec!["info".to_string()],
            }
        }
    }

    impl TestConfig {
        pub fn new() -> Self {
            ConfigBuilder::new(TestCliEnv::new()).build().unwrap()
        }
    }

    impl ConfigExt for TestConfig {
        const FILENAME: &'static str = "wavs.toml";

        fn with_data_dir(&mut self, f: fn(&mut PathBuf)) {
            f(&mut self.data);
        }

        fn log_levels(&self) -> impl Iterator<Item = &str> {
            self.log_level.iter().map(|s| s.as_str())
        }
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    struct TestCliEnv {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub home: Option<PathBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub dotenv: Option<PathBuf>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        #[serde(deserialize_with = "deserialize_vec_string")]
        pub log_level: Vec<String>,
    }

    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn utils_path() -> PathBuf {
        workspace_path().join("packages").join("utils")
    }

    impl TestCliEnv {
        pub fn new() -> Self {
            Self {
                home: Some(workspace_path().join("packages").join("wavs")),
                // this purposefully points at a non-existing file
                // so that we don't load a real .env in tests
                dotenv: Some(utils_path().join("does-not-exist")),
                log_level: Vec::new(),
            }
        }
    }

    impl CliEnvExt for TestCliEnv {
        const ENV_VAR_PREFIX: &'static str = "WAVS";

        fn home_dir(&self) -> Option<PathBuf> {
            self.home.clone()
        }

        fn dotenv_path(&self) -> Option<PathBuf> {
            self.dotenv.clone()
        }
    }

    // this test is confiming the user overrides for filepath work as expected
    // but it does not test the complete list of fallbacks past those first few common cases
    // because the complete list will change depending on the platform, global env vars, etc.
    #[tokio::test]
    async fn config_filepath() {
        fn filepaths(home: Option<PathBuf>) -> Vec<PathBuf> {
            let home = TestCliEnv {
                home,
                dotenv: None,
                log_level: Vec::new(),
            }
            .merge_cli_env_args()
            .unwrap()
            .home;

            ConfigFilePath::new(TestConfig::FILENAME, home).into_possible()
        }

        // make sure all the test directories are not there by default
        let default_dirs = filepaths(None);
        for i in 1..=10 {
            assert!(!default_dirs
                .contains(&PathBuf::from(format!("/tmp{}", i)).join(TestConfig::FILENAME)));
        }

        // if provide a specific home directory, then it is the first one to try
        assert_eq!(
            filepaths(Some("/tmp1".into())).first().unwrap(),
            &PathBuf::from("/tmp1").join(TestConfig::FILENAME)
        );

        // even if we also provide it in an env var, it still takes precedence
        temp_env::with_vars(
            [(
                format!("{}_{}", TestCliEnv::ENV_VAR_PREFIX, "HOME"),
                Some("/tmp2"),
            )],
            || {
                assert_eq!(
                    filepaths(Some("/tmp3".into())).first().unwrap(),
                    &PathBuf::from("/tmp3").join(TestConfig::FILENAME)
                );
            },
        );

        // but if we provide an env var, and not a specific home directory, then env var becomes the first
        temp_env::with_vars(
            [(
                format!("{}_{}", TestCliEnv::ENV_VAR_PREFIX, "HOME"),
                Some("/tmp2"),
            )],
            || {
                assert_eq!(
                    filepaths(None).first().unwrap(),
                    &PathBuf::from("/tmp2").join(TestConfig::FILENAME),
                );
            },
        );
    }

    // tests that default values are set correctly
    #[tokio::test]
    async fn config_default() {
        // port is *not* set in the test toml file
        assert_eq!(TestConfig::default().port, TestConfig::new().port);
    }

    // tests that we can configure array-strings, and it overrides as expected
    #[tokio::test]
    async fn config_array_string() {
        static TRACING_ENV_FILTER_ENV: LazyLock<tracing_subscriber::EnvFilter> =
            LazyLock::new(|| {
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("debug".parse().unwrap())
                    .add_directive("foo=trace".parse().unwrap())
            });
        static TRACING_ENV_FILTER_CLI: LazyLock<tracing_subscriber::EnvFilter> =
            LazyLock::new(|| {
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("trace".parse().unwrap())
                    .add_directive("bar=debug".parse().unwrap())
            });

        let config = temp_env::with_vars(
            [(
                format!("{}_{}", TestCliEnv::ENV_VAR_PREFIX, "LOG_LEVEL"),
                Some("info, wavs=debug, just_to_confirm_test=debug"),
            )],
            TestConfig::new,
        );

        assert_eq!(
            config.log_level,
            ["info", "wavs=debug", "just_to_confirm_test=debug"]
        );

        // replace the var and check that it is now what we expect
        // env replacement needs to be in an async function
        {
            temp_env::async_with_vars(
                [(
                    format!("{}_{}", TestCliEnv::ENV_VAR_PREFIX, "LOG_LEVEL"),
                    Some("debug, foo=trace"),
                )],
                check(),
            )
            .await;

            async fn check() {
                // first - if we don't set a CLI var, it should use the env var
                let config = TestConfig::new();
                assert_eq!(
                    config.tracing_env_filter().unwrap().to_string(),
                    TRACING_ENV_FILTER_ENV.to_string()
                );

                // but then, even when the env var is available, if we set a CLI var, it should override
                let mut cli_args = TestCliEnv::new();
                cli_args.log_level = TRACING_ENV_FILTER_CLI
                    .to_string()
                    .split(",")
                    .map(|s| s.to_string())
                    .collect();

                let config: TestConfig = ConfigBuilder::new(cli_args).build().unwrap();

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
        let mut cli_args = TestCliEnv::new();

        cli_args.dotenv = Some(utils_path().join("tests").join(".env.test"));

        let _ = ConfigBuilder::<TestConfig, _>::new(cli_args)
            .build()
            .unwrap();

        // if we try to check against meaningful env vars, we may conflict with other tests and/or user settings
        // so just check for a dummy value since this test only cares about the dotenv file itself
        // coverage of environment var overrides is in other tests with temp_env scopes
        assert_eq!(
            std::env::var(format!("{}_RANDOM_TEST_VALUE", TestCliEnv::ENV_VAR_PREFIX)).unwrap(),
            "hello world"
        );

        // unset the value, just to play nice, though this could be a race condition (see docs on remove_var)
        std::env::remove_var(format!("{}_RANDOM_TEST_VALUE", TestCliEnv::ENV_VAR_PREFIX))
    }

    #[test]
    fn chain_name_lookup() {
        let chain_configs = mock_chain_configs();
        let chain: CosmosChainConfig = chain_configs
            .get_chain(&"cosmos".try_into().unwrap())
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(chain.chain_id, "cosmos");

        let chain: EthereumChainConfig = chain_configs
            .get_chain(&"eth".try_into().unwrap())
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(chain.chain_id, "eth");
    }

    #[test]
    fn chain_name_lookup_fails_duplicate() {
        let mut chain_configs = mock_chain_configs();

        chain_configs.cosmos.insert(
            "eth".try_into().unwrap(),
            CosmosChainConfig {
                chain_id: "eth".to_string(),
                bech32_prefix: "eth".to_string(),
                rpc_endpoint: Some("http://127.0.0.1:1317".to_string()),
                grpc_endpoint: Some("http://127.0.0.1:9090".to_string()),
                gas_price: 0.01,
                gas_denom: "uatom".to_string(),
                faucet_endpoint: Some("http://127.0.0.1:8000".to_string()),
            },
        );

        assert!(chain_configs.get_chain(&"eth".try_into().unwrap()).is_err());
    }

    fn mock_chain_configs() -> ChainConfigs {
        ChainConfigs {
            cosmos: vec![
                (
                    "cosmos".try_into().unwrap(),
                    CosmosChainConfig {
                        chain_id: "cosmos".to_string(),
                        bech32_prefix: "cosmos".to_string(),
                        rpc_endpoint: Some("http://127.0.0.1:1317".to_string()),
                        grpc_endpoint: Some("http://127.0.0.1:9090".to_string()),
                        gas_price: 0.01,
                        gas_denom: "uatom".to_string(),
                        faucet_endpoint: Some("http://127.0.0.1:8000".to_string()),
                    },
                ),
                (
                    "layer".try_into().unwrap(),
                    CosmosChainConfig {
                        chain_id: "layer".to_string(),
                        bech32_prefix: "layer".to_string(),
                        rpc_endpoint: Some("http://127.0.0.1:1317".to_string()),
                        grpc_endpoint: Some("http://127.0.0.1:9090".to_string()),
                        gas_price: 0.01,
                        gas_denom: "uatom".to_string(),
                        faucet_endpoint: Some("http://127.0.0.1:8000".to_string()),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            eth: vec![
                (
                    "eth".try_into().unwrap(),
                    EthereumChainConfig {
                        chain_id: "eth".to_string(),
                        ws_endpoint: Some("ws://127.0.0.1:8546".to_string()),
                        http_endpoint: Some("http://127.0.0.1:8545".to_string()),
                        aggregator_endpoint: Some("http://127.0.0.1:8000".to_string()),
                        faucet_endpoint: Some("http://127.0.0.1:8000".to_string()),
                    },
                ),
                (
                    "polygon".try_into().unwrap(),
                    EthereumChainConfig {
                        chain_id: "polygon".to_string(),
                        ws_endpoint: Some("ws://127.0.0.1:8546".to_string()),
                        http_endpoint: Some("http://127.0.0.1:8545".to_string()),
                        aggregator_endpoint: Some("http://127.0.0.1:8000".to_string()),
                        faucet_endpoint: Some("http://127.0.0.1:8000".to_string()),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}
