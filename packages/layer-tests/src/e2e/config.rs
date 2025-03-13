use utils::{
    config::{ChainConfigs, ConfigBuilder, CosmosChainConfig, EthereumChainConfig},
    filesystem::workspace_path,
};
use wavs_types::ChainName;

use crate::config::TestConfig;

use super::matrix::TestMatrix;

#[derive(Clone, Debug)]
pub struct Configs {
    pub matrix: TestMatrix,
    pub wavs: wavs::config::Config,
    pub cli: wavs_cli::config::Config,
    pub cli_args: wavs_cli::args::CliArgs,
    pub aggregator: Option<wavs_aggregator::config::Config>,
    pub chains: ChainConfigs,
}

impl From<TestConfig> for Configs {
    fn from(test_config: TestConfig) -> Self {
        let matrix = test_config
            .matrix
            .into_validated(test_config.all, test_config.isolated.as_deref());

        let mut chain_configs = ChainConfigs::default();

        let mut eth_port = 8545;
        let mut eth_chain_id = 31337;

        let mut push_eth_chain = |aggregator: bool| {
            let http_endpoint = format!("http://127.0.0.1:{}", eth_port);
            let ws_endpoint = format!("ws://127.0.0.1:{}", eth_port);
            let chain_id = eth_chain_id.to_string();

            let chain_config = EthereumChainConfig {
                chain_id: chain_id.to_string(),
                http_endpoint: Some(http_endpoint),
                ws_endpoint: Some(ws_endpoint),
                aggregator_endpoint: if aggregator {
                    Some("http://127.0.0.1:8001".to_string())
                } else {
                    None
                },
                faucet_endpoint: None,
            };

            chain_configs
                .eth
                .insert(ChainName::new(chain_id).unwrap(), chain_config);

            eth_port += 1;
            eth_chain_id += 1;
        };

        let mut cosmos_port = 9545;
        let mut cosmos_chain_id = 1;

        let mut push_cosmos_chain = |_aggregator: bool| {
            let rpc_endpoint = format!("http://127.0.0.1:{}", cosmos_port);
            let chain_id = format!("wasmd-{}", cosmos_chain_id);

            let chain_config = CosmosChainConfig {
                chain_id: chain_id.to_string(),
                rpc_endpoint: Some(rpc_endpoint),
                grpc_endpoint: None,
                gas_price: 0.025,
                gas_denom: "ucosm".to_string(),
                bech32_prefix: "wasm".to_string(),
                faucet_endpoint: None,
            };

            chain_configs
                .cosmos
                .insert(ChainName::new(chain_id).unwrap(), chain_config);

            cosmos_port += 1;
            cosmos_chain_id += 1;
        };

        if matrix.eth_regular_chain_enabled() {
            push_eth_chain(false);
        }

        if matrix.eth_secondary_chain_enabled() {
            push_eth_chain(false);
        }

        if matrix.eth_aggregator_chain_enabled() {
            push_eth_chain(true);
        }

        if matrix.cosmos_regular_chain_enabled() {
            push_cosmos_chain(false);
        }

        let mut wavs_config: wavs::config::Config = ConfigBuilder::new(wavs::args::CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(workspace_path().join("packages").join("wavs")),
            // deliberately point to a non-existing file
            dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .unwrap();

        wavs_config.active_trigger_chains = chain_configs.all_chain_names();

        wavs_config.chains = chain_configs.clone();

        let aggregator_config = if matrix.eth_aggregator_chain_enabled() {
            let mut aggregator_config: wavs_aggregator::config::Config =
                ConfigBuilder::new(wavs_aggregator::args::CliArgs {
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
            aggregator_config.chain = chain_configs.eth.keys().last().cloned().unwrap();

            Some(aggregator_config)
        } else {
            None
        };

        let cli_args = wavs_cli::args::CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(workspace_path().join("packages").join("cli")),
            // deliberately point to a non-existing file
            dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
            ..Default::default()
        };

        let mut cli_config: wavs_cli::config::Config =
            ConfigBuilder::new(cli_args.clone()).build().unwrap();

        cli_config.chains = chain_configs.clone();

        // Sanity check

        if let Some(aggregator_config) = aggregator_config.as_ref() {
            let aggregator_endpoint = format!(
                "http://{}:{}",
                aggregator_config.host, aggregator_config.port
            );
            for eth_chain in chain_configs.eth.values() {
                if let Some(endpoint) = eth_chain.aggregator_endpoint.as_ref() {
                    assert_eq!(*endpoint, aggregator_endpoint);
                }
            }
        }

        Self {
            matrix,
            cli: cli_config,
            cli_args,
            aggregator: aggregator_config,
            wavs: wavs_config,
            chains: chain_configs,
        }
    }
}
