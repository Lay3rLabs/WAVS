use super::solidity_types::{
    stake_registry::ECDSAStakeRegistry::{self, Quorum, StrategyParams},
    token::{IStrategy, LayerToken},
};
use crate::{
    alloy_helpers::SolidityEventFinder,
    avs_client::{
        layer_service_manager::LayerServiceManager,
        stake_registry::ISignatureUtils::SignatureWithSaltAndExpiry,
    },
    eigen_client::{
        avs_deploy::setup_empty_proxy,
        solidity_types::{
            misc::{StrategyFactory, StrategyManager::StrategyAddedToDepositWhitelist},
            proxy::ProxyAdmin,
            BoxSigningProvider, ProxyAdminT,
        },
        CoreAVSAddresses,
    },
    eth_client::EthSigningClient,
};
use alloy::{
    primitives::{aliases::U96, Address, FixedBytes, TxHash, U256},
    signers::SignerSync,
    sol_types::SolCall,
};
use anyhow::{Context, Result};
use chrono::Utc;
use rand::prelude::*;

pub struct AvsClient {
    pub eth: EthSigningClient,
    pub core: CoreAVSAddresses,
    pub service_manager: Address,
}

impl AvsClient {
    pub async fn register_operator(&self, rng: &mut impl Rng) -> Result<TxHash> {
        let mut salt = [0u8; 32];
        rng.fill_bytes(&mut salt);

        let salt = FixedBytes::from_slice(&salt);
        let now = Utc::now().timestamp();
        let expiry: U256 = U256::from(now + 3600);

        let digest_hash = self
            .core
            .calculate_operator_avs_registration_digest_hash(
                self.eth.address(),
                self.service_manager,
                salt,
                expiry,
                self.eth.provider.clone(),
            )
            .await?;

        let signature = self.eth.signer.sign_hash_sync(&digest_hash)?;
        let operator_signature = SignatureWithSaltAndExpiry {
            signature: signature.as_bytes().into(),
            salt,
            expiry,
        };

        let stake_registry_address =
            LayerServiceManager::new(self.service_manager, &self.eth.provider)
                .stakeRegistry()
                .call()
                .await?
                ._0;

        let contract_ecdsa_stake_registry =
            ECDSAStakeRegistry::new(stake_registry_address, self.eth.provider.clone());

        let register_operator_hash = contract_ecdsa_stake_registry
            .registerOperatorWithSignature(operator_signature, self.eth.signer.clone().address())
            .gas(500000)
            .send()
            .await?
            .get_receipt()
            .await?
            .transaction_hash;

        tracing::debug!(
            "Operator registered on AVS successfully :{} , tx_hash :{}",
            self.eth.signer.address(),
            register_operator_hash
        );
        Ok(register_operator_hash)
    }
}

pub struct AvsClientDeployer {
    pub eth: EthSigningClient,
    pub core_avs_addrs: Option<CoreAVSAddresses>,
}

impl AvsClientDeployer {
    pub fn new(eth: EthSigningClient) -> Self {
        Self {
            eth,
            core_avs_addrs: None,
        }
    }

    pub fn core_addresses(mut self, addresses: CoreAVSAddresses) -> Self {
        self.core_avs_addrs = Some(addresses);
        self
    }
}

pub struct StrategyAndToken {
    pub token: Address,
    pub quorum: Quorum,
}

pub struct ServiceManagerDeps {
    pub provider: BoxSigningProvider,
    pub avs_directory: Address,
    pub stake_registry: Address,
    pub rewards_coordinator: Address,
    pub delegation_manager: Address,
}

impl AvsClientDeployer {
    pub async fn deploy_strategy_and_token(
        &self,
        strategy_factory: Address,
    ) -> Result<StrategyAndToken> {
        let token = LayerToken::deploy(self.eth.provider.clone()).await?;
        let strategy_factory = StrategyFactory::new(strategy_factory, self.eth.provider.clone());

        let tx_receipt = strategy_factory
            .deployNewStrategy(*token.address())
            .send()
            .await?
            .get_receipt()
            .await?;

        // https://github.com/Layr-Labs/eigenlayer-contracts/blob/e4c66a62923f6844edb7684803f575abd5381634/src/contracts/core/StrategyManager.sol#L187
        let strategy_added: StrategyAddedToDepositWhitelist = tx_receipt
            .solidity_event()
            .context("No strategy address found")?;

        Ok(StrategyAndToken {
            token: *token.address(),
            quorum: Quorum {
                strategies: vec![StrategyParams {
                    strategy: strategy_added.strategy,
                    multiplier: U96::from(10_000_u64),
                }],
            },
        })
    }

    pub async fn deploy_service_manager(
        mut self,
        payload_handler: Address,
        setup: Option<StrategyAndToken>,
    ) -> Result<AvsClient> {
        let core = self.core_avs_addrs.take().context("AVS Core must be set")?;
        let proxies = Proxies::new(&self.eth).await?;
        let setup = match setup {
            Some(setup) => setup,
            None => {
                self.deploy_strategy_and_token(core.strategy_factory)
                    .await?
            }
        };

        // sanity check - we own the ProxyAdmin
        debug_assert_eq!(proxies.admin.owner().call().await?._0, self.eth.address());

        let strategy = IStrategy::new(
            setup.quorum.strategies.first().as_ref().unwrap().strategy,
            self.eth.provider.clone(),
        );

        // get ecdsa_stake_registry
        let impl_contract =
            ECDSAStakeRegistry::deploy(self.eth.provider.clone(), core.delegation_manager).await?;

        proxies
            .admin
            .upgradeAndCall(
                proxies.ecdsa_stake_registry,
                *impl_contract.address(),
                ECDSAStakeRegistry::initializeCall {
                    _serviceManager: proxies.service_manager,
                    _thresholdWeight: U256::ZERO,
                    _quorum: setup.quorum,
                }
                .abi_encode()
                .into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        // Get service manager
        let service_manager = LayerServiceManager::deploy(
            self.eth.provider.clone(),
            core.avs_directory,
            proxies.ecdsa_stake_registry,
            core.rewards_coordinator,
            core.delegation_manager,
            payload_handler,
        )
        .await?;

        let service_manager_address = *service_manager.address();

        proxies
            .admin
            .upgrade(proxies.service_manager, service_manager_address)
            .send()
            .await?
            .watch()
            .await?;

        let underlying_token = strategy.underlyingToken().call().await?._0;
        assert_ne!(underlying_token, Address::ZERO);
        tracing::debug!("underlying strategy token addr: {}", underlying_token);

        Ok(AvsClient {
            eth: self.eth,
            core,
            service_manager: proxies.service_manager,
        })
    }

    pub async fn into_client(mut self, service_manager: Address) -> Result<AvsClient> {
        let core = self.core_avs_addrs.take().context("AVS Core must be set")?;

        Ok(AvsClient {
            eth: self.eth,
            core,
            service_manager,
        })
    }
}

struct Proxies {
    pub admin: ProxyAdminT,
    pub service_manager: Address,
    pub ecdsa_stake_registry: Address,
}

impl Proxies {
    pub async fn new(eth: &EthSigningClient) -> Result<Self> {
        let admin = ProxyAdmin::deploy(eth.provider.clone()).await?;

        tracing::debug!("Eigen core proxy admin: {}", admin.address());

        Ok(Self {
            ecdsa_stake_registry: setup_empty_proxy(eth, &admin).await?,
            service_manager: setup_empty_proxy(eth, &admin).await?,
            admin,
        })
    }
}
