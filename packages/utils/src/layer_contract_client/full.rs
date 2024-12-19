use super::solidity_types::{
    layer_service_manager::LayerServiceManager,
    layer_trigger::LayerTrigger,
    stake_registry::ECDSAStakeRegistry::{self, Quorum, StrategyParams},
    token::{IStrategy, LayerToken},
};
use crate::{
    alloy_helpers::SolidityEventFinder,
    eigen_client::{
        avs_deploy::setup_empty_proxy,
        solidity_types::{
            misc::{StrategyFactory, StrategyManager::StrategyAddedToDepositWhitelist},
            proxy::ProxyAdmin,
            ProxyAdminT,
        },
        CoreAVSAddresses,
    },
    eth_client::EthSigningClient,
    layer_contract_client::stake_registry::ISignatureUtils::SignatureWithSaltAndExpiry,
};
use alloy::{
    primitives::{aliases::U96, Address, FixedBytes, TxHash, U256},
    signers::SignerSync,
    sol_types::SolCall,
};
use anyhow::{Context, Result};
use chrono::Utc;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

pub struct LayerContractClientFull {
    pub eth: EthSigningClient,
    pub core: CoreAVSAddresses,
    pub layer: LayerAddresses,
}

impl LayerContractClientFull {
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
                self.layer.service_manager,
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

        let contract_ecdsa_stake_registry =
            ECDSAStakeRegistry::new(self.layer.stake_registry, self.eth.provider.clone());

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LayerAddresses {
    pub proxy_admin: Address,
    pub service_manager: Address,
    pub trigger: Address,
    pub stake_registry: Address,
    pub token: Address,
}

pub struct LayerContractClientFullBuilder {
    pub eth: EthSigningClient,
    pub core_avs_addrs: Option<CoreAVSAddresses>,
}

impl LayerContractClientFullBuilder {
    pub fn new(eth: EthSigningClient) -> Self {
        Self {
            eth,
            core_avs_addrs: None,
        }
    }

    pub fn avs_addresses(mut self, addresses: CoreAVSAddresses) -> Self {
        self.core_avs_addrs = Some(addresses);
        self
    }
}

struct SetupAddrs {
    pub token: Address,
    pub quorum: Quorum,
}

impl LayerContractClientFullBuilder {
    async fn set_up(&self, strategy_factory: Address) -> Result<SetupAddrs> {
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

        Ok(SetupAddrs {
            token: *token.address(),
            quorum: Quorum {
                strategies: vec![StrategyParams {
                    strategy: strategy_added.strategy,
                    multiplier: U96::from(10_000_u64),
                }],
            },
        })
    }

    pub async fn build(mut self) -> Result<LayerContractClientFull> {
        let core = self.core_avs_addrs.take().context("AVS Core must be set")?;
        let proxies = Proxies::new(&self.eth).await?;
        let setup = self.set_up(core.strategy_factory).await?;

        // sanity check - we own the ProxyAdmin
        debug_assert_eq!(proxies.admin.owner().call().await?._0, self.eth.address());

        let strategy = IStrategy::new(
            setup.quorum.strategies.first().as_ref().unwrap().strategy,
            self.eth.provider.clone(),
        );

        tracing::debug!("deploying ECDSA stake registry");
        let ecdsa_stake_registry_impl =
            ECDSAStakeRegistry::deploy(self.eth.provider.clone(), core.delegation_manager).await?;

        tracing::debug!("deploying Hello world registry");
        let service_manager_impl = LayerServiceManager::deploy(
            self.eth.provider.clone(),
            core.avs_directory,
            proxies.ecdsa_stake_registry,
            core.rewards_coordinator,
            core.delegation_manager,
        )
        .await?;

        let upgrade_call = ECDSAStakeRegistry::initializeCall {
            _serviceManager: proxies.service_manager,
            _thresholdWeight: U256::ZERO,
            _quorum: setup.quorum,
        };

        tracing::debug!("Upgrading stake registry");
        proxies
            .admin
            .upgradeAndCall(
                proxies.ecdsa_stake_registry,
                *ecdsa_stake_registry_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        tracing::debug!("Upgrading hello world");
        proxies
            .admin
            .upgrade(proxies.service_manager, *service_manager_impl.address())
            .send()
            .await?
            .watch()
            .await?;

        let underlying_token = strategy.underlyingToken().call().await?._0;
        assert_ne!(underlying_token, Address::ZERO);
        tracing::debug!("underlying strategy token addr: {}", underlying_token);

        let trigger = LayerTrigger::deploy(self.eth.provider.clone()).await?;
        let trigger_address = *trigger.address();

        // Upgrade contracts
        Ok(LayerContractClientFull {
            eth: self.eth,
            core,
            layer: LayerAddresses {
                proxy_admin: *proxies.admin.address(),
                service_manager: proxies.service_manager,
                stake_registry: proxies.ecdsa_stake_registry,
                token: setup.token,
                trigger: trigger_address,
            },
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
