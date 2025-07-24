use std::num::NonZeroU32;

use alloy_signer_local::{coins_bip39::English, MnemonicBuilder};
use utils::{
    config::{ChainConfigs, ConfigBuilder, CosmosChainConfig, EvmChainConfig, EvmChainConfigExt},
    evm_client::EvmSigningClient,
    filesystem::workspace_path,
};
use wavs_types::ChainName;

use crate::config::TestConfig;

use super::matrix::TestMatrix;

pub const DEFAULT_CHAIN_ID: u64 = 31337;
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
}

#[derive(Clone, Debug)]
pub struct TestMnemonics {
    pub cli: String,
    pub wavs: String,
    pub aggregator: String,
}

impl TestMnemonics {
    pub fn new() -> Self {
        // just some random mnemonics so they don't conflict with binaries, we'll fund it from the anvil wallet upon creation
        Self {
            // 0x63A513A1c878283BC1fF829d6938f45D714E22A1
            cli: "replace course few short practice end crawl element rather strong text fit"
                .to_string(),
            // 0x55a8F5cac28c2dA45aFA89c46e47CC4A445570AE
            wavs: "aspect mushroom fly cousin hobby body need dose blind siren shoe annual"
                .to_string(),
            // 0xB1Ebb71428FF42F529708B5Afd2BA6Ad3432f38d
            aggregator:
                "brain medal write network foam renew muscle mirror rather daring bike uniform"
                    .to_string(),
        }
    }

    pub async fn fund(&self, chain_configs: &ChainConfigs) {
        for chain_config in chain_configs.evm.values() {
            let anvil_mnemonic =
                "test test test test test test test test test test test junk".to_string();
            let anvil_config = chain_config.signing_client_config(anvil_mnemonic).unwrap();
            let anvil_client = EvmSigningClient::new(anvil_config).await.unwrap();

            for mnemonic in [&self.cli, &self.wavs, &self.aggregator] {
                let dest_addr = MnemonicBuilder::<English>::default()
                    .phrase(mnemonic)
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
        let mut evm_chain_id = DEFAULT_CHAIN_ID;

        let mut push_evm_chain = || {
            let http_endpoint = format!("http://127.0.0.1:{}", evm_port);
            let ws_endpoint = format!("ws://127.0.0.1:{}", evm_port);
            let chain_id = evm_chain_id.to_string();

            let chain_config = EvmChainConfig {
                chain_id: chain_id.to_string(),
                http_endpoint: Some(http_endpoint),
                ws_endpoint: Some(ws_endpoint),
                faucet_endpoint: None,
                poll_interval_ms: None,
            };

            chain_configs
                .evm
                .insert(ChainName::new(chain_id).unwrap(), chain_config);

            evm_port += 1;
            evm_chain_id += 1;
        };

        let mut cosmos_port = 9545;
        let mut cosmos_chain_id = 1;

        let mut push_cosmos_chain = || {
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
            aggregators: vec![aggregator_config],
            wavs: wavs_config,
            chains: chain_configs,
            mnemonics,
        }
    }
}
