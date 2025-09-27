use std::{num::NonZeroU32, sync::LazyLock};

use alloy_signer_local::{coins_bip39::English, MnemonicBuilder};
use utils::{
    config::{
        ChainConfigs, ConfigBuilder, CosmosChainConfigBuilder, EvmChainConfigBuilder,
        EvmChainConfigExt,
    },
    evm_client::EvmSigningClient,
    filesystem::workspace_path,
};
use wavs_types::{ChainKey, Credential};

use crate::config::TestConfig;

use super::matrix::TestMatrix;

// Aggregator endpoint configuration
pub const AGGREGATOR_HOST: &str = "127.0.0.1";
pub const AGGREGATOR_PORT_1: u32 = 8001;
pub const AGGREGATOR_PORT_2: u32 = 8002;

pub fn aggregator_endpoint_1() -> String {
    format!("http://{}:{}", AGGREGATOR_HOST, AGGREGATOR_PORT_1)
}

pub fn aggregator_endpoint_2() -> String {
    format!("http://{}:{}", AGGREGATOR_HOST, AGGREGATOR_PORT_2)
}

pub static DEFAULT_CHAIN_KEY: LazyLock<ChainKey> = LazyLock::new(|| "evm:31337".parse().unwrap());
pub const CRON_INTERVAL_DATA: &str = "cron-interval data";
// we can go down to 1 for small groups of tests, but it currently causes a long wait in the test runner
// might be a good candidate to use this a a benchmark for increasing throughput
pub static BLOCK_INTERVAL: NonZeroU32 = NonZeroU32::new(10).unwrap();

#[derive(Clone, Debug)]
pub struct Configs {
    pub matrix: TestMatrix,
    pub registry: bool,
    pub wavs: wavs::config::Config,
    pub cli: wavs_cli::config::Config,
    pub cli_args: wavs_cli::args::CliArgs,
    pub aggregators: Vec<wavs_aggregator::config::Config>,
    pub chains: ChainConfigs,
    pub mnemonics: TestMnemonics,
    pub middleware_concurrency: bool,
    pub wavs_concurrency: bool,
}

#[derive(Clone, Debug)]
pub struct TestMnemonics {
    pub cli: Credential,
    pub wavs: Credential,
    pub aggregator: Credential,
    pub aggregator_2: Credential,
}

impl TestMnemonics {
    pub fn new() -> Self {
        // just some random mnemonics so they don't conflict with binaries, we'll fund it from the anvil wallet upon creation
        Self {
            // 0x63A513A1c878283BC1fF829d6938f45D714E22A1
            cli: Credential::new(
                "replace course few short practice end crawl element rather strong text fit"
                    .to_string(),
            ),
            // 0x55a8F5cac28c2dA45aFA89c46e47CC4A445570AE
            wavs: Credential::new(
                "aspect mushroom fly cousin hobby body need dose blind siren shoe annual"
                    .to_string(),
            ),
            // 0xB1Ebb71428FF42F529708B5Afd2BA6Ad3432f38d
            aggregator: Credential::new(
                "brain medal write network foam renew muscle mirror rather daring bike uniform"
                    .to_string(),
            ),
            // 0x5E661B79FE2D3F6cE70F5AAC07d8Cd9AF2161630
            aggregator_2: Credential::new(
                "candy maple cake sugar pudding cream honey rich smooth crumble sweet treat"
                    .to_string(),
            ),
        }
    }

    pub async fn fund(&self, chain_configs: &ChainConfigs) {
        for chain_config in chain_configs.evm_iter() {
            let anvil_mnemonic =
                "test test test test test test test test test test test junk".to_string();
            let anvil_config = chain_config
                .signing_client_config(Credential::new(anvil_mnemonic))
                .unwrap();
            let anvil_client = EvmSigningClient::new(anvil_config).await.unwrap();

            for mnemonic in [&self.cli, &self.wavs, &self.aggregator, &self.aggregator_2] {
                let dest_addr = MnemonicBuilder::<English>::default()
                    .phrase(mnemonic.as_str())
                    .build()
                    .unwrap()
                    .address();

                anvil_client.transfer_funds(dest_addr, "100").await.unwrap();
            }
        }
    }
}

impl From<TestConfig> for Configs {
    fn from(test_config: TestConfig) -> Self {
        let matrix: TestMatrix = test_config.mode.into();

        let mnemonics = TestMnemonics::new();

        let mut chain_configs = ChainConfigs::default();

        let mut evm_port = 8545;
        let mut evm_chain_id = DEFAULT_CHAIN_KEY.id.to_string().parse::<u32>().unwrap();

        let mut push_evm_chain = || {
            let http_endpoint = format!("http://127.0.0.1:{}", evm_port);
            let ws_endpoint = format!("ws://127.0.0.1:{}", evm_port);

            let chain_config = EvmChainConfigBuilder {
                http_endpoint: Some(http_endpoint),
                ws_endpoint: Some(ws_endpoint),
                faucet_endpoint: None,
                poll_interval_ms: None,
                channel_size: None,
            };

            chain_configs
                .evm
                .insert(evm_chain_id.to_string().parse().unwrap(), chain_config);

            evm_port += 1;
            evm_chain_id += 1;
        };

        let mut cosmos_port = 9545;
        let mut cosmos_chain_id = 1;

        let mut push_cosmos_chain = || {
            let rpc_endpoint = format!("http://127.0.0.1:{}", cosmos_port);

            let chain_config = CosmosChainConfigBuilder {
                rpc_endpoint: Some(rpc_endpoint),
                grpc_endpoint: None,
                gas_price: 0.025,
                gas_denom: "ucosm".to_string(),
                bech32_prefix: "wasm".to_string(),
                faucet_endpoint: None,
            };

            chain_configs.cosmos.insert(
                format!("wasmd-{}", cosmos_chain_id).parse().unwrap(),
                chain_config,
            );

            cosmos_port += 1;
            cosmos_chain_id += 1;
        };

        if matrix.evm_regular_chain_enabled() {
            push_evm_chain();
        }

        if matrix.evm_secondary_chain_enabled() {
            push_evm_chain();
        }

        if matrix.cosmos_regular_chain_enabled() {
            push_cosmos_chain();
        }

        let mut wavs_config: wavs::config::Config = ConfigBuilder::new(wavs::args::CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(workspace_path()),
            // deliberately point to a non-existing file
            dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .unwrap();

        wavs_config.chains = chain_configs.clone();
        wavs_config.submission_mnemonic = Some(mnemonics.wavs.clone());
        wavs_config.dev_endpoints_enabled = true;

        // Create first aggregator config (default port 8001)
        let mut aggregator_config: wavs_aggregator::config::Config =
            ConfigBuilder::new(wavs_aggregator::args::CliArgs {
                data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
                home: Some(workspace_path()),
                // deliberately point to a non-existing file
                dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
                ..Default::default()
            })
            .build()
            .unwrap();

        aggregator_config.chains = chain_configs.clone();
        aggregator_config.credential = Some(mnemonics.aggregator.clone());
        aggregator_config.dev_endpoints_enabled = true;

        // Create second aggregator config
        // It is used only in few tests, but we need to spin it beforehand
        let mut aggregator_config_2 = aggregator_config.clone();
        aggregator_config_2.port = AGGREGATOR_PORT_2;
        aggregator_config_2.data = tempfile::tempdir().unwrap().path().to_path_buf();
        aggregator_config_2.credential = Some(mnemonics.aggregator_2.clone());
        aggregator_config_2.dev_endpoints_enabled = true;

        let cli_args = wavs_cli::args::CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(workspace_path()),
            // deliberately point to a non-existing file
            dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
            ..Default::default()
        };

        let mut cli_config: wavs_cli::config::Config =
            ConfigBuilder::new(cli_args.clone()).build().unwrap();

        cli_config.chains = chain_configs.clone();
        // some random mnemonic
        cli_config.evm_credential = Some(mnemonics.cli.clone());

        Self {
            matrix,
            registry: test_config.registry.map_or_else(|| false, |t| t),
            cli: cli_config,
            cli_args,
            aggregators: vec![aggregator_config, aggregator_config_2],
            wavs: wavs_config,
            chains: chain_configs,
            mnemonics,
            middleware_concurrency: test_config.middleware_concurrency,
            wavs_concurrency: test_config.wavs_concurrency,
        }
    }
}
