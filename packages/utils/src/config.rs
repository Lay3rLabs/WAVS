use anyhow::{bail, Context, Result};
use figment::{providers::Format, Figment};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::BTreeMap, marker::PhantomData, path::PathBuf};
use utoipa::ToSchema;

use crate::{
    error::EvmClientError,
    evm_client::{EvmEndpoint, EvmSigningClientConfig},
};
pub use wavs_types::AnyChainConfig;
use wavs_types::{ChainConfigError, ChainKey, ChainKeyId, ChainKeyNamespace, Credential};

pub const WAVS_ENV_PREFIX: &str = "WAVS_ENV";

/// The builder we use to build Config
#[derive(Debug)]
pub struct ConfigBuilder<CONFIG, ARG> {
    pub cli_env_args: ARG,
    _config: PhantomData<CONFIG>,
}

pub trait CliEnvExt: Serialize + DeserializeOwned + Default + std::fmt::Debug {
    // e.g. "WAVS"
    const ENV_VAR_PREFIX: &'static str;

    // The section identifier in the TOML file, e.g. "wavs", "cli", "aggregator"
    const TOML_IDENTIFIER: &'static str;

    // whether to print debug messages during config loading
    const PRINT_DEBUG_MSGS: bool = false;

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
    const FILENAME: &'static str = "wavs.toml";

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
            if ARG::PRINT_DEBUG_MSGS {
                eprintln!("Loading env vars from {}", dotenv_path.display());
            }
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

        if ARG::PRINT_DEBUG_MSGS {
            eprintln!("Loading config from {}", filepath.display());
        }

        let figment = Figment::new()
            // Start with the default values as the base
            .merge(figment::providers::Serialized::defaults(CONFIG::default()))
            // Then add default section from TOML
            .merge(Figment::from(
                figment::providers::Toml::file(&filepath).nested(),
            ))
            // Then add specific section, overriding globals where needed
            .merge(
                Figment::from(figment::providers::Toml::file(&filepath).nested())
                    .select(ARG::TOML_IDENTIFIER),
            )
            // Finally override with cli/env args
            .merge(figment::providers::Serialized::defaults(cli_env_args));

        // Extract the config
        let mut config: CONFIG = figment.extract()?;

        config.with_data_dir(|data_dir| {
            *data_dir = shellexpand::tilde(&data_dir.to_string_lossy())
                .to_string()
                .into();
        });

        if ARG::PRINT_DEBUG_MSGS {
            config.with_data_dir(|data_dir| {
                eprintln!("Using data directory: {}", data_dir.display());
            });
        }

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
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(format!(".{DIRNAME}"))) {
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

/// Chains are identified by `ChainKey`, which is a combination of a namespace and id
/// for now - we natively support 3 namespaces: cosmos, evm, and dev
#[derive(Debug, Default, Deserialize, Serialize, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct ChainConfigs {
    /// Cosmos-style chains (including Layer-SDK)
    pub cosmos: BTreeMap<ChainKeyId, CosmosChainConfigBuilder>,
    /// EVM-style chains
    pub evm: BTreeMap<ChainKeyId, EvmChainConfigBuilder>,
    /// DEV-only chains
    /// The key here can be different than the chain_id inside the config
    pub dev: BTreeMap<ChainKeyId, AnyChainConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct CosmosChainConfigBuilder {
    pub bech32_prefix: String,
    pub rpc_endpoint: Option<String>,
    pub grpc_endpoint: Option<String>,
    pub gas_price: f32,
    pub gas_denom: String,
    pub faucet_endpoint: Option<String>,
}

impl CosmosChainConfigBuilder {
    pub fn build(self, id: ChainKeyId) -> CosmosChainConfig {
        CosmosChainConfig {
            chain_id: id,
            bech32_prefix: self.bech32_prefix,
            rpc_endpoint: self.rpc_endpoint,
            grpc_endpoint: self.grpc_endpoint,
            gas_price: self.gas_price,
            gas_denom: self.gas_denom,
            faucet_endpoint: self.faucet_endpoint,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct EvmChainConfigBuilder {
    pub ws_endpoints: Vec<String>,
    pub http_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    pub ws_priority_endpoint_index: Option<usize>,
}

impl EvmChainConfigBuilder {
    pub fn build(self, id: ChainKeyId) -> EvmChainConfig {
        EvmChainConfig {
            chain_id: id,
            ws_endpoints: self.ws_endpoints,
            http_endpoint: self.http_endpoint,
            faucet_endpoint: self.faucet_endpoint,
            ws_priority_endpoint_index: self.ws_priority_endpoint_index,
        }
    }
}

impl ChainConfigs {
    pub fn get_chain(&self, key: &ChainKey) -> Option<AnyChainConfig> {
        match key.namespace.as_str() {
            ChainKeyNamespace::COSMOS => self
                .cosmos
                .get(&key.id)
                .map(|c| AnyChainConfig::Cosmos(c.clone().build(key.id.clone()))),
            ChainKeyNamespace::EVM => self
                .evm
                .get(&key.id)
                .map(|c| AnyChainConfig::Evm(c.clone().build(key.id.clone()))),
            ChainKeyNamespace::DEV => self.dev.get(&key.id).cloned(),
            _ => None,
        }
    }

    pub fn cosmos_iter(&self) -> impl Iterator<Item = CosmosChainConfig> + '_ {
        self.cosmos
            .iter()
            .map(|(id, config)| config.clone().build(id.clone()))
    }

    pub fn evm_iter(&self) -> impl Iterator<Item = EvmChainConfig> + '_ {
        self.evm
            .iter()
            .map(|(id, config)| config.clone().build(id.clone()))
    }

    pub fn dev_iter(&self) -> impl Iterator<Item = AnyChainConfig> + '_ {
        self.dev.values().cloned()
    }

    pub fn all_chain_keys(&self) -> Result<Vec<ChainKey>, anyhow::Error> {
        let mut keys = Vec::new();

        for id in self.evm.keys() {
            keys.push(ChainKey {
                namespace: ChainKeyNamespace::EVM.parse()?,
                id: id.clone(),
            });
        }
        for id in self.cosmos.keys() {
            keys.push(ChainKey {
                namespace: ChainKeyNamespace::COSMOS.parse()?,
                id: id.clone(),
            });
        }
        for id in self.dev.keys() {
            keys.push(ChainKey {
                namespace: ChainKeyNamespace::DEV.parse()?,
                id: id.clone(),
            });
        }

        Ok(keys)
    }

    pub fn chain_keys(&self, namespace: ChainKeyNamespace) -> Vec<ChainKey> {
        match namespace.as_str() {
            ChainKeyNamespace::COSMOS => self
                .cosmos
                .keys()
                .map(|id| ChainKey {
                    namespace: namespace.clone(),
                    id: id.clone(),
                })
                .collect(),
            ChainKeyNamespace::EVM => self
                .evm
                .keys()
                .map(|id| ChainKey {
                    namespace: namespace.clone(),
                    id: id.clone(),
                })
                .collect(),
            ChainKeyNamespace::DEV => self
                .dev
                .keys()
                .map(|id| ChainKey {
                    namespace: namespace.clone(),
                    id: id.clone(),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn add_chain(
        &mut self,
        key: ChainKey,
        config: AnyChainConfig,
    ) -> Result<(), ChainConfigError> {
        if self.get_chain(&key).is_some() {
            return Err(ChainConfigError::DuplicateChain(key));
        }

        match key.namespace.as_str() {
            ChainKeyNamespace::DEV => {
                self.dev.insert(key.id, config);
            }
            ChainKeyNamespace::EVM => match config {
                AnyChainConfig::Evm(evm_config) => {
                    if evm_config.chain_id != key.id {
                        return Err(ChainConfigError::IdMismatch {
                            expected: key.id,
                            actual: evm_config.chain_id,
                        });
                    }
                    let evm_config = EvmChainConfigBuilder {
                        ws_endpoints: evm_config.ws_endpoints,
                        http_endpoint: evm_config.http_endpoint,
                        faucet_endpoint: evm_config.faucet_endpoint,
                        ws_priority_endpoint_index: evm_config.ws_priority_endpoint_index,
                    };
                    self.evm.insert(key.id, evm_config);
                }
                _ => return Err(ChainConfigError::InvalidNamespaceForEvm(key.namespace)),
            },
            ChainKeyNamespace::COSMOS => match config {
                AnyChainConfig::Cosmos(cosmos_config) => {
                    if cosmos_config.chain_id != key.id {
                        return Err(ChainConfigError::IdMismatch {
                            expected: key.id,
                            actual: cosmos_config.chain_id,
                        });
                    }
                    let cosmos_config = CosmosChainConfigBuilder {
                        bech32_prefix: cosmos_config.bech32_prefix,
                        rpc_endpoint: cosmos_config.rpc_endpoint,
                        grpc_endpoint: cosmos_config.grpc_endpoint,
                        gas_price: cosmos_config.gas_price,
                        gas_denom: cosmos_config.gas_denom,
                        faucet_endpoint: cosmos_config.faucet_endpoint,
                    };
                    self.cosmos.insert(key.id, cosmos_config);
                }
                _ => return Err(ChainConfigError::InvalidNamespaceForCosmos(key.namespace)),
            },
            _ => return Err(ChainConfigError::InvalidNamespace(key.namespace)),
        }

        Ok(())
    }
}

pub use wavs_types::{CosmosChainConfig, EvmChainConfig};

pub trait EvmChainConfigExt {
    fn signing_client_config(
        &self,
        credential: Credential,
    ) -> std::result::Result<EvmSigningClientConfig, EvmClientError>;
    fn query_client_endpoints(&self) -> std::result::Result<Vec<EvmEndpoint>, EvmClientError>;
}

impl EvmChainConfigExt for EvmChainConfig {
    fn signing_client_config(
        &self,
        credential: Credential,
    ) -> std::result::Result<EvmSigningClientConfig, EvmClientError> {
        // TODO: https://github.com/Lay3rLabs/WAVS/issues/1019
        let endpoint = match (self.ws_endpoints.is_empty(), self.http_endpoint.clone()) {
            // prefer HTTP for signing clients
            (_, Some(url)) => EvmEndpoint::new_http(&url)?,
            (false, _) => EvmEndpoint::new_ws(&self.ws_endpoints[0])?,
            _ => {
                return Err(EvmClientError::ParseEndpoint(
                    "No endpoint provided".to_string(),
                ));
            }
        };

        let config = EvmSigningClientConfig::new(endpoint, credential);

        Ok(config)
    }

    fn query_client_endpoints(&self) -> std::result::Result<Vec<EvmEndpoint>, EvmClientError> {
        match (self.ws_endpoints.is_empty(), self.http_endpoint.clone()) {
            // prefer WS for query clients
            (false, _) => self
                .ws_endpoints
                .iter()
                .map(|url| EvmEndpoint::new_ws(url))
                .collect(),
            (_, Some(url)) => Ok(vec![EvmEndpoint::new_http(&url)?]),
            _ => Err(EvmClientError::ParseEndpoint(
                "No endpoint provided".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{collections::BTreeMap, path::PathBuf, sync::LazyLock};

    use serde::{Deserialize, Serialize};
    use wavs_types::{ChainConfigError, ChainKey};

    use crate::{
        config::{ConfigFilePath, CosmosChainConfigBuilder, EvmChainConfigBuilder},
        filesystem::workspace_path,
        serde::deserialize_vec_string,
    };

    use super::{
        ChainConfigs, CliEnvExt, ConfigBuilder, ConfigExt, CosmosChainConfig, EvmChainConfig,
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

    fn utils_path() -> PathBuf {
        workspace_path().join("packages").join("utils")
    }

    impl TestCliEnv {
        pub fn new() -> Self {
            Self {
                home: Some(workspace_path()),
                // this purposefully points at a non-existing file
                // so that we don't load a real .env in tests
                dotenv: Some(utils_path().join("does-not-exist")),
                log_level: Vec::new(),
            }
        }
    }

    impl CliEnvExt for TestCliEnv {
        const ENV_VAR_PREFIX: &'static str = "WAVS";
        const TOML_IDENTIFIER: &'static str = "test";

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
                .contains(&PathBuf::from(format!("/tmp{i}")).join(TestConfig::FILENAME)));
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
            .get_chain(&"cosmos:layer".try_into().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(chain.chain_id.as_str(), "layer");

        let chain: EvmChainConfig = chain_configs
            .get_chain(&"evm:anvil".try_into().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(chain.chain_id.as_str(), "anvil");
    }

    #[tokio::test]
    async fn test_service_specific_overrides() {
        // Define a test config structure
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct ServiceConfig {
            pub data: PathBuf,
            pub chains: ChainConfigs,
        }

        impl Default for ServiceConfig {
            fn default() -> Self {
                Self {
                    data: PathBuf::from("/var/service"),
                    chains: ChainConfigs::default(),
                }
            }
        }

        impl ConfigExt for ServiceConfig {
            const FILENAME: &'static str = "test_wavs.toml";

            fn with_data_dir(&mut self, f: fn(&mut PathBuf)) {
                f(&mut self.data);
            }

            fn log_levels(&self) -> impl Iterator<Item = &str> {
                [].iter().copied()
            }
        }

        // Define CLI args structure for the test
        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        #[serde(default)]
        struct ServiceCliEnv {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub home: Option<PathBuf>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub dotenv: Option<PathBuf>,
        }

        impl CliEnvExt for ServiceCliEnv {
            const ENV_VAR_PREFIX: &'static str = "SERVICE";
            const TOML_IDENTIFIER: &'static str = "service1";

            fn home_dir(&self) -> Option<PathBuf> {
                self.home.clone()
            }

            fn dotenv_path(&self) -> Option<PathBuf> {
                self.dotenv.clone()
            }
        }

        // Create test config file with global and service-specific overrides
        let test_config = r#"
    # Global chain config
    [chains.evm.global_chain]
    ws_endpoints = ["ws://global.example.com"]
    http_endpoint = "http://global.example.com"

    # Service1 specific settings
    [service1]
    data = "/var/service1"

    # Service1 specific chain override
    [service1.chains.evm.global_chain]
    ws_endpoints = ["ws://service1.example.com"]
    http_endpoint = "http://service1.example.com"

    # Service1 specific chain that doesn't exist in global
    [service1.chains.evm.service1_chain]
    ws_endpoints = ["ws://service1-special.example.com"]
    http_endpoint = "http://service1-special.example.com"

    # Service2 specific settings
    [service2]
    data = "/var/service2"

    # Service2 specific chain override
    [service2.chains.evm.global_chain]
    ws_endpoints = ["ws://service2.example.com"]
    http_endpoint = "http://service2.example.com"
    "#;

        // Write test config file
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(ServiceConfig::FILENAME);
        std::fs::write(&config_path, test_config).unwrap();

        // Setup CLI env for service1
        let service1_cli_env = ServiceCliEnv {
            home: Some(temp_dir.path().to_path_buf()),
            dotenv: None,
        };

        // Load service1 config
        let service1_config: ServiceConfig = ConfigBuilder::new(service1_cli_env.clone())
            .build()
            .unwrap();

        // Define expected chain configurations
        let global_chain_key: ChainKey = "evm:global_chain".try_into().unwrap();
        let service1_chain_key: ChainKey = "evm:service1_chain".try_into().unwrap();

        // Test global chain with service1 overrides
        let global_chain_config = service1_config.chains.get_chain(&global_chain_key).unwrap();

        if let crate::config::AnyChainConfig::Evm(evm_config) = global_chain_config {
            assert_eq!(evm_config.chain_id.as_str(), "global_chain");
            // These should be overridden by service1
            assert_eq!(evm_config.ws_endpoints[0], "ws://service1.example.com");
            assert_eq!(
                evm_config.http_endpoint.as_deref(),
                Some("http://service1.example.com")
            );
        } else {
            panic!("Expected EVM chain config");
        }

        // Test service1-specific chain
        let service1_chain_config = service1_config
            .chains
            .get_chain(&service1_chain_key)
            .unwrap();

        if let crate::config::AnyChainConfig::Evm(evm_config) = service1_chain_config {
            assert_eq!(evm_config.chain_id.as_str(), "service1_chain");
            assert_eq!(
                evm_config.ws_endpoints[0],
                "ws://service1-special.example.com"
            );
            assert_eq!(
                evm_config.http_endpoint.as_deref(),
                Some("http://service1-special.example.com")
            );
        } else {
            panic!("Expected EVM chain config");
        }

        // Now test with a different service profile
        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        #[serde(default)]
        struct Service2CliEnv {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub home: Option<PathBuf>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub dotenv: Option<PathBuf>,
        }

        impl CliEnvExt for Service2CliEnv {
            const ENV_VAR_PREFIX: &'static str = "SERVICE";
            const TOML_IDENTIFIER: &'static str = "service2";

            fn home_dir(&self) -> Option<PathBuf> {
                self.home.clone()
            }

            fn dotenv_path(&self) -> Option<PathBuf> {
                self.dotenv.clone()
            }
        }

        let service2_cli_env = Service2CliEnv {
            home: Some(temp_dir.path().to_path_buf()),
            dotenv: None,
        };

        // Define a generic type that can be used for both service configs
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct Service2Config {
            pub data: PathBuf,
            pub chains: ChainConfigs,
        }

        impl Default for Service2Config {
            fn default() -> Self {
                Self {
                    data: PathBuf::from("/var/service"),
                    chains: ChainConfigs::default(),
                }
            }
        }

        impl ConfigExt for Service2Config {
            const FILENAME: &'static str = "test_wavs.toml";

            fn with_data_dir(&mut self, f: fn(&mut PathBuf)) {
                f(&mut self.data);
            }

            fn log_levels(&self) -> impl Iterator<Item = &str> {
                [].iter().copied()
            }
        }

        // Load service2 config
        let service2_config: Service2Config = ConfigBuilder::new(service2_cli_env).build().unwrap();

        // Test global chain with service2 overrides
        let global_chain_config = service2_config.chains.get_chain(&global_chain_key).unwrap();

        if let crate::config::AnyChainConfig::Evm(evm_config) = global_chain_config {
            assert_eq!(evm_config.chain_id.as_str(), "global_chain");
            assert_eq!(evm_config.ws_endpoints[0], "ws://service2.example.com");
            assert_eq!(
                evm_config.http_endpoint.as_deref(),
                Some("http://service2.example.com")
            );
        } else {
            panic!("Expected EVM chain config");
        }

        // Test that service2 doesn't have the service1-specific chain
        assert!(service2_config
            .chains
            .get_chain(&service1_chain_key)
            .is_none());

        // Test data_dir override for different services
        assert_eq!(service1_config.data, PathBuf::from("/var/service1"));
        assert_eq!(service2_config.data, PathBuf::from("/var/service2"));
    }

    #[test]
    fn chain_configs_toml() {
        let test_config = r#"
            [evm.1]
            ws_endpoints = ["ws://example-1.com", "ws://example-1-alt.com"]
            http_endpoint = "http://example-1.com"

            [evm.2]
            ws_endpoints = ["ws://example-2.com"]
            http_endpoint = "http://example-2.com"

            [cosmos.neutron]
            bech32_prefix = "neutron"
            rpc_endpoint = "https://rpc-falcron.pion-1.ntrn.tech"
            grpc_endpoint = "http://grpc-falcron.pion-1.ntrn.tech:80"
            gas_price = 0.0053
            gas_denom = "untrn"

            [cosmos.layer]
            bech32_prefix = "layer"
            rpc_endpoint = "http://localhost:26657"
            grpc_endpoint = "http://localhost:9090"
            gas_price = 0.025
            gas_denom = "uslay"

            [dev.my-local-evm-1]
            type = "evm"
            chain_id = "1"
            ws_endpoints = ["ws://example-local-evm-1.com", "ws://example-local-evm-1-alt.com"]
            http_endpoint = "http://example-local-evm-1.com"

            [dev.my-local-evm-2]
            type = "evm"
            chain_id = "2"
            ws_endpoints = ["ws://example-local-evm-2.com"]
            http_endpoint = "http://example-local-evm-2.com"

            [dev.my-local-cosmos-1]
            type = "cosmos"
            chain_id = "wasmd"
            bech32_prefix = "wasmd"
            rpc_endpoint = "http://example-local-cosmos-1.com"
            grpc_endpoint = "https://example-local-cosmos-1.com"
            gas_price = 0.025
            gas_denom = "uwasm1"

            [dev.my-local-cosmos-2]
            type = "cosmos"
            chain_id = "wasmd"
            bech32_prefix = "wasmd"
            rpc_endpoint = "http://example-local-cosmos-2.com"
            grpc_endpoint = "https://example-local-cosmos-2.com"
            gas_price = 0.025
            gas_denom = "uwasm2"
        "#;

        let chain_configs: ChainConfigs = toml::from_str(test_config).unwrap();

        let mut keys: Vec<String> = chain_configs
            .all_chain_keys()
            .unwrap()
            .into_iter()
            .map(|x| x.to_string())
            .collect();

        keys.sort();

        assert_eq!(
            keys,
            [
                "cosmos:layer",
                "cosmos:neutron",
                "dev:my-local-cosmos-1",
                "dev:my-local-cosmos-2",
                "dev:my-local-evm-1",
                "dev:my-local-evm-2",
                "evm:1",
                "evm:2"
            ]
        );

        // load evm
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("evm:1").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .http_endpoint
                .unwrap(),
            "http://example-1.com"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("evm:1").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .ws_endpoints,
            vec!["ws://example-1.com", "ws://example-1-alt.com"]
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("evm:1").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .chain_id
                .as_str(),
            "1"
        );

        // distinguish evm
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("evm:2").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .http_endpoint
                .unwrap(),
            "http://example-2.com"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("evm:2").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .ws_endpoints,
            vec!["ws://example-2.com"]
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("evm:2").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .chain_id
                .as_str(),
            "2"
        );

        // load cosmos
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("cosmos:layer").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .gas_denom,
            "uslay"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("cosmos:layer").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .chain_id
                .as_str(),
            "layer"
        );

        // distinguish cosmos
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("cosmos:neutron").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .gas_denom,
            "untrn"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("cosmos:neutron").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .chain_id
                .as_str(),
            "neutron"
        );

        // load dev evm
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-evm-1").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .http_endpoint
                .unwrap(),
            "http://example-local-evm-1.com"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-evm-1").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .ws_endpoints,
            vec![
                "ws://example-local-evm-1.com",
                "ws://example-local-evm-1-alt.com"
            ]
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-evm-1").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .chain_id
                .as_str(),
            "1"
        );

        // distinguish dev evm
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-evm-2").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .http_endpoint
                .unwrap(),
            "http://example-local-evm-2.com"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-evm-2").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .ws_endpoints,
            vec!["ws://example-local-evm-2.com"]
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-evm-2").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap()
                .chain_id
                .as_str(),
            "2"
        );

        // load dev cosmos
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-cosmos-1").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .gas_denom,
            "uwasm1"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-cosmos-1").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .chain_id
                .as_str(),
            "wasmd"
        );

        // distinguish dev cosmos
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-cosmos-2").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .gas_denom,
            "uwasm2"
        );

        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-cosmos-2").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap()
                .chain_id
                .as_str(),
            "wasmd"
        );

        // fail to get eth dev as cosmos
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-evm-1").unwrap())
                .unwrap()
                .to_cosmos_config()
                .unwrap_err(),
            ChainConfigError::ExpectedCosmosChain
        );

        // fail to get cosmos dev as eth
        assert_eq!(
            chain_configs
                .get_chain(&ChainKey::try_from("dev:my-local-cosmos-1").unwrap())
                .unwrap()
                .to_evm_config()
                .unwrap_err(),
            ChainConfigError::ExpectedEvmChain
        );
    }

    fn mock_chain_configs() -> ChainConfigs {
        ChainConfigs {
            cosmos: vec![
                (
                    "wasmd".try_into().unwrap(),
                    CosmosChainConfigBuilder {
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
                    CosmosChainConfigBuilder {
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
            evm: vec![
                (
                    "anvil".try_into().unwrap(),
                    EvmChainConfigBuilder {
                        ws_endpoints: vec!["ws://127.0.0.1:8546".to_string()],
                        http_endpoint: Some("http://127.0.0.1:8545".to_string()),
                        faucet_endpoint: Some("http://127.0.0.1:8000".to_string()),
                        ws_priority_endpoint_index: None,
                    },
                ),
                (
                    "polygon".try_into().unwrap(),
                    EvmChainConfigBuilder {
                        ws_endpoints: vec!["ws://127.0.0.1:8546".to_string()],
                        http_endpoint: Some("http://127.0.0.1:8545".to_string()),
                        faucet_endpoint: Some("http://127.0.0.1:8000".to_string()),
                        ws_priority_endpoint_index: None,
                    },
                ),
            ]
            .into_iter()
            .collect(),
            dev: BTreeMap::new(),
        }
    }
}
