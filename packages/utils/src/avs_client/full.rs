use super::solidity_types::{
    layer_service_manager::LayerServiceManager,
    stake_registry::ECDSAStakeRegistry::{self, Quorum, StrategyParams},
    token::{IStrategy, LayerToken},
};
use crate::{
    alloy_helpers::SolidityEventFinder,
    avs_client::stake_registry::ISignatureUtils::SignatureWithSaltAndExpiry,
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
use serde::{Deserialize, Serialize};

pub struct AvsClient {
    pub eth: EthSigningClient,
    pub core: CoreAVSAddresses,
    pub layer: AvsAddresses,
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
pub struct AvsAddresses {
    pub proxy_admin: Address,
    pub service_manager: Address,
    pub stake_registry: Address,
    pub token: Address,
}

impl AvsAddresses {
    pub fn as_vec(&self) -> Vec<Address> {
        vec![
            self.proxy_admin,
            self.service_manager,
            self.stake_registry,
            self.token,
        ]
    }
}

pub struct AvsClientBuilder {
    pub eth: EthSigningClient,
    pub core_avs_addrs: Option<CoreAVSAddresses>,

    /// if set, this service manager will be used instead of the default
    /// LayerServiceManager contract
    service_manager: Option<Address>,
}

impl AvsClientBuilder {
    pub fn new(eth: EthSigningClient) -> Self {
        Self {
            eth,
            core_avs_addrs: None,
            service_manager: None,
        }
    }

    pub fn core_addresses(mut self, addresses: CoreAVSAddresses) -> Self {
        self.core_avs_addrs = Some(addresses);
        self
    }

    // if your service manager is already deployed, you can override it here to use it.
    pub fn override_service_manager(mut self, service_manager: Option<Address>) -> Self {
        self.service_manager = service_manager;
        self
    }
}

struct SetupAddrs {
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

impl AvsClientBuilder {
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

    pub async fn build<F, Fut>(mut self, deploy_service_manager: F) -> Result<AvsClient>
    where
        F: FnOnce(ServiceManagerDeps) -> Fut,
        Fut: std::future::Future<Output = Result<Address>>,
    {
        let core = self.core_avs_addrs.take().context("AVS Core must be set")?;
        let proxies = Proxies::new(&self.eth).await?;
        let setup = self.set_up(core.strategy_factory).await?;

        // sanity check - we own the ProxyAdmin
        debug_assert_eq!(proxies.admin.owner().call().await?._0, self.eth.address());

        let strategy = IStrategy::new(
            setup.quorum.strategies.first().as_ref().unwrap().strategy,
            self.eth.provider.clone(),
        );

        // Get or deploy stake registry
        let ecdsa_stake_registry_address =
            if let Some(service_manager_address) = self.service_manager {
                LayerServiceManager::new(service_manager_address, self.eth.provider.clone())
                    .stakeRegistry()
                    .call()
                    .await?
                    ._0
            } else {
                let impl_contract =
                    ECDSAStakeRegistry::deploy(self.eth.provider.clone(), core.delegation_manager)
                        .await?;
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
                proxies.ecdsa_stake_registry
            };

        // Get or deploy service manager
        let service_manager_address = match self.service_manager {
            Some(addr) => addr,
            None => {
                let service_manager_addr = deploy_service_manager(ServiceManagerDeps {
                    provider: self.eth.provider.clone(),
                    avs_directory: core.avs_directory,
                    stake_registry: proxies.ecdsa_stake_registry,
                    rewards_coordinator: core.rewards_coordinator,
                    delegation_manager: core.delegation_manager,
                })
                .await?;

                proxies
                    .admin
                    .upgrade(proxies.service_manager, service_manager_addr)
                    .send()
                    .await?
                    .watch()
                    .await?;

                proxies.service_manager
            }
        };

        let underlying_token = strategy.underlyingToken().call().await?._0;
        assert_ne!(underlying_token, Address::ZERO);
        tracing::debug!("underlying strategy token addr: {}", underlying_token);

        Ok(AvsClient {
            eth: self.eth,
            core,
            layer: AvsAddresses {
                proxy_admin: *proxies.admin.address(),
                service_manager: service_manager_address,
                stake_registry: ecdsa_stake_registry_address,
                token: setup.token,
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
