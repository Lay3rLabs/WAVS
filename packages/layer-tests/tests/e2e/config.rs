use utils::{
    config::{ChainConfigs, ConfigBuilder, CosmosChainConfig, EthereumChainConfig},
    filesystem::workspace_path,
};

#[derive(Clone, Debug)]
pub struct Configs {
    pub wavs: wavs::config::Config,
    pub cli: wavs_cli::config::Config,
    pub aggregator: aggregator::config::Config,
    pub chains: ChainConfigs,
}

impl Configs {
    pub fn new(
        eth_chains: Vec<EthereumChainConfig>,
        cosmos_chains: Vec<CosmosChainConfig>,
    ) -> Self {
        let chain_configs = ChainConfigs {
            cosmos: cosmos_chains
                .iter()
                .map(|chain_config| (chain_config.chain_id.clone(), chain_config.clone()))
                .collect(),
            eth: eth_chains
                .iter()
                .map(|chain_config| (chain_config.chain_id.clone(), chain_config.clone()))
                .collect(),
        };

        let mut wavs_config: wavs::config::Config = ConfigBuilder::new(wavs::args::CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(workspace_path().join("packages").join("wavs")),
            // deliberately point to a non-existing file
            dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .unwrap();

        wavs_config.eth_chains = eth_chains
            .iter()
            .map(|chain_config| chain_config.chain_id.clone())
            .collect();
        wavs_config.cosmos_chain = cosmos_chains
            .first()
            .map(|chain_config| chain_config.chain_id.clone());
        wavs_config.chains = chain_configs.clone();

        let mut aggregator_config: aggregator::config::Config =
            ConfigBuilder::new(aggregator::args::CliArgs {
                data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
                home: Some(workspace_path().join("packages").join("aggregator")),
                // deliberately point to a non-existing file
                dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
                ..Default::default()
            })
            .build()
            .unwrap();

        aggregator_config.chains = chain_configs.clone();
        aggregator_config.chain = eth_chains
            .first()
            .map(|chain_config| chain_config.chain_id.clone())
            .unwrap();

        let mut cli_config: wavs_cli::config::Config =
            ConfigBuilder::new(wavs_cli::args::CliArgs {
                data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
                home: Some(workspace_path().join("packages").join("cli")),
                // deliberately point to a non-existing file
                dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
                ..Default::default()
            })
            .build()
            .unwrap();

        cli_config.chains = chain_configs.clone();

        Self {
            cli: cli_config,
            aggregator: aggregator_config,
            wavs: wavs_config,
            chains: chain_configs,
        }
    }
}
