use rand::prelude::*;
use std::{
    num::NonZeroU32,
    sync::{Arc, RwLock},
};

use alloy_signer_local::{coins_bip39::English, MnemonicBuilder};
use utils::{
    config::{ConfigBuilder, EvmChainConfigExt},
    evm_client::EvmSigningClient,
    filesystem::workspace_path,
    test_utils::middleware::evm::EvmMiddlewareType,
};
use wavs::subsystems::aggregator::p2p::P2pConfig;
use wavs_types::{ChainConfigs, CosmosChainConfigBuilder, Credential, EvmChainConfigBuilder};

use crate::config::TestConfig;

use super::matrix::TestMatrix;

pub const CRON_INTERVAL_DATA: &str = "cron-interval data";
// we can go down to 1 for small groups of tests, but it currently causes a long wait in the test runner
// might be a good candidate to use this a a benchmark for increasing throughput
pub static BLOCK_INTERVAL: NonZeroU32 = NonZeroU32::new(10).unwrap();

#[derive(Clone, Debug)]
pub struct Configs {
    pub matrix: TestMatrix,
    pub registry: bool,
    /// WAVS configs - one per operator node
    pub wavs_configs: Vec<wavs::config::Config>,
    pub cli: wavs_cli::config::Config,
    pub cli_args: wavs_cli::args::CliArgs,
    pub chains: Arc<RwLock<ChainConfigs>>,
    pub mnemonics: TestMnemonics,
    pub middleware_concurrency: bool,
    pub wavs_concurrency: bool,
    pub grouping: bool,
    pub evm_middleware_type: EvmMiddlewareType,
}

impl Configs {
    /// Number of operator nodes configured
    pub fn num_operators(&self) -> usize {
        self.wavs_configs.len()
    }
}

#[derive(Clone, Debug)]
pub struct TestMnemonics {
    pub cli: Credential,
    pub cli_cosmos: Credential,
    /// Operator mnemonics - one per WAVS instance for multi-operator tests
    /// For single-operator tests, only the first one is used
    pub operators: Vec<Credential>,
    pub aggregator_evm: Credential,
    pub aggregator_cosmos: Credential,
    pub cosmos_middleware: Vec<Credential>,
}

impl TestMnemonics {
    pub fn new(num_operators: usize) -> Self {
        // just some random mnemonics so they don't conflict with binaries, we'll fund it from the anvil wallet upon creation

        // Pre-defined operator mnemonics for multi-operator tests
        // Each operator needs a unique mnemonic to have a unique signing key
        let operator_mnemonics = vec![
            // Operator 0: 0x55a8F5cac28c2dA45aFA89c46e47CC4A445570AE
            "aspect mushroom fly cousin hobby body need dose blind siren shoe annual",
            // Operator 1
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            // Operator 2
            "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
        ];

        let operators: Vec<Credential> = operator_mnemonics
            .into_iter()
            .take(num_operators)
            .map(|m| Credential::new(m.to_string()))
            .collect();

        Self {
            // 0x63A513A1c878283BC1fF829d6938f45D714E22A1
            cli: Credential::new(
                "replace course few short practice end crawl element rather strong text fit"
                    .to_string(),
            ),
            cli_cosmos: Credential::new(
                "arch forward congress comfort shove palace staff flat concert such double tooth brown buffalo cycle school change exhaust episode ball embody various enroll tenant"
                    .to_string(),
            ),
            operators,
            // 0xB1Ebb71428FF42F529708B5Afd2BA6Ad3432f38d
            aggregator_evm: Credential::new(
                "brain medal write network foam renew muscle mirror rather daring bike uniform"
                    .to_string(),
            ),
            aggregator_cosmos: Credential::new(
                "body total lion ritual invest cup destroy kidney fame symptom gasp snake spy between wire style elegant walk furnace total verify clay swamp flavor"
                    .to_string(),
            ),
            cosmos_middleware: vec![],
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

            // Collect all mnemonics to fund
            let mut mnemonics_to_fund: Vec<&Credential> =
                vec![&self.cli, &self.aggregator_evm, &self.aggregator_cosmos];

            // Add all operator mnemonics
            for operator in &self.operators {
                mnemonics_to_fund.push(operator);
            }

            for mnemonic in mnemonics_to_fund {
                let dest_addr = MnemonicBuilder::<English>::default()
                    .phrase(mnemonic.as_str())
                    .build()
                    .unwrap()
                    .address();

                anvil_client.transfer_funds(dest_addr, "100").await.unwrap();
            }
        }
    }

    pub fn push_cosmos_middleware(&mut self) {
        let mut rng = rand::rng();

        let entropy: [u8; 32] = rng.random();
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap().to_string();
        self.cosmos_middleware.push(Credential::new(mnemonic));
    }
}

/// Number of operators for multi-operator tests
pub const MULTI_OPERATOR_COUNT: usize = 3;
/// Base port for WAVS HTTP servers
pub const WAVS_BASE_PORT: u32 = 8000;

impl From<TestConfig> for Configs {
    fn from(test_config: TestConfig) -> Self {
        let matrix: TestMatrix = test_config.mode.into();

        // Determine number of operators based on test matrix
        let num_operators = if matrix.multi_operator_enabled() {
            MULTI_OPERATOR_COUNT
        } else {
            1
        };

        let mut mnemonics = TestMnemonics::new(num_operators);

        let chain_configs = Arc::new(RwLock::new(ChainConfigs::default()));

        let mut evm_port = 8545;
        let mut evm_chain_id = 31337;

        let mut push_evm_chain = || {
            let http_endpoint = format!("http://127.0.0.1:{}", evm_port);
            let ws_endpoint = format!("ws://127.0.0.1:{}", evm_port);

            let chain_config = EvmChainConfigBuilder {
                http_endpoint: Some(http_endpoint),
                ws_endpoints: vec![ws_endpoint],
                faucet_endpoint: None,
                ws_priority_endpoint_index: None,
            };

            chain_configs
                .write()
                .unwrap()
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

            chain_configs.write().unwrap().cosmos.insert(
                format!("wasmd-{}", cosmos_chain_id).parse().unwrap(),
                chain_config,
            );

            mnemonics.push_cosmos_middleware();

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

        // Create WAVS configs for each operator
        let mut wavs_configs = Vec::with_capacity(num_operators);
        for operator_index in 0..num_operators {
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
            // Each operator gets its own signing mnemonic for unique signing keys
            wavs_config.signing_mnemonic = Some(mnemonics.operators[operator_index].clone());
            wavs_config.aggregator_cosmos_credential = Some(mnemonics.aggregator_cosmos.clone());
            wavs_config.aggregator_evm_credential = Some(mnemonics.aggregator_evm.clone());
            wavs_config.dev_endpoints_enabled = true;
            // Each operator gets a unique port
            wavs_config.port = WAVS_BASE_PORT + operator_index as u32;

            wavs_configs.push(wavs_config);
        }

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
        cli_config.cosmos_mnemonic = Some(mnemonics.cli_cosmos.clone());

        Self {
            matrix,
            registry: test_config.registry.unwrap_or(false),
            cli: cli_config,
            cli_args,
            wavs_configs,
            chains: chain_configs,
            mnemonics,
            middleware_concurrency: test_config.middleware_concurrency,
            wavs_concurrency: test_config.wavs_concurrency,
            grouping: test_config.grouping,
            evm_middleware_type: test_config.middleware_type,
        }
    }
}
