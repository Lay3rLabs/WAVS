use utils::{
    config::{ChainConfigs, ConfigBuilder, CosmosChainConfig, EthereumChainConfig},
    filesystem::workspace_path,
};

use crate::config::TestConfig;

#[derive(Clone, Debug)]
pub struct Configs {
    pub test_config: TestConfig,
    pub wavs: wavs::config::Config,
    pub cli: wavs_cli::config::Config,
    pub aggregator: Option<aggregator::config::Config>,
    pub chains: ChainConfigs,
}

impl Configs {
    pub fn new(
        test_config: TestConfig,
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

        wavs_config.active_trigger_chains = eth_chains
            .iter()
            .map(|chain_config| chain_config.chain_id.clone())
            .chain(
                cosmos_chains
                    .iter()
                    .map(|chain_config| chain_config.chain_id.clone()),
            )
            .collect();

        wavs_config.chains = chain_configs.clone();

        let aggregator_config = if test_config.matrix.eth.aggregator_chain_enabled() {
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

            // for now, we just assume it's always the last eth chain...
            // down the line, we might want to make this configurable
            aggregator_config.chain = eth_chains
                .last()
                .map(|chain_config| chain_config.chain_id.clone())
                .unwrap();

            Some(aggregator_config)
        } else {
            None
        };

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

        // Sanity check

        if let Some(aggregator_config) = aggregator_config.as_ref() {
            let aggregator_endpoint = format!(
                "http://{}:{}",
                aggregator_config.host, aggregator_config.port
            );
            for eth_chain in eth_chains.iter() {
                if let Some(endpoint) = eth_chain.aggregator_endpoint.as_ref() {
                    assert_eq!(*endpoint, aggregator_endpoint);
                }
            }
        }

        Self {
            test_config,
            cli: cli_config,
            aggregator: aggregator_config,
            wavs: wavs_config,
            chains: chain_configs,
        }
    }
}
