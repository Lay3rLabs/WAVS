use alloy::{
    primitives::{aliases::U96, Address, U256},
    sol_types::SolCall,
};

use crate::{
    alloy_helpers::SolidityEventFinder,
    eigen_client::{
        avs_deploy::{setup_empty_proxy, ProxyAdminT},
        solidity_types::{
            misc::{StrategyFactory, StrategyManager::StrategyAddedToDepositWhitelist},
            proxy::ProxyAdmin,
        },
    },
    eth_client::EthSigningClient,
    hello_world::{
        config::HelloWorldAddressesConfig,
        solidity_types::{
            hello_world::HelloWorldServiceManager,
            stake_registry::ECDSAStakeRegistry::{self, Quorum, StrategyParams},
            token::IStrategy,
        },
    },
};

use super::{
    config::HelloWorldDeployment, solidity_types::token::LayerToken, HelloWorldClient,
    HelloWorldClientBuilder,
};
use anyhow::{Context, Result};

struct SetupAddrs {
    pub token: Address,
    pub quorum: Quorum,
}

impl HelloWorldClientBuilder {
    async fn set_up(&self, strategy_factory: Address) -> Result<SetupAddrs> {
        tracing::debug!("setting up");
        let token = LayerToken::deploy(self.eth.http_provider.clone()).await?;
        tracing::debug!("deployed token: {}", token.address());
        let strategy_factory =
            StrategyFactory::new(strategy_factory, self.eth.http_provider.clone());

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

    pub async fn build(mut self) -> Result<HelloWorldClient> {
        tracing::debug!("Building");
        let core = self.core_avs_addrs.take().context("AVS Core must be set")?;
        let proxies = Proxies::new(&self.eth).await?;
        let setup = self.set_up(core.strategy_factory).await?;

        // sanity check - we own the ProxyAdmin
        debug_assert_eq!(proxies.admin.owner().call().await?._0, self.eth.address());

        let strategy = IStrategy::new(
            setup.quorum.strategies.first().as_ref().unwrap().strategy,
            self.eth.http_provider.clone(),
        );

        tracing::debug!("deploying ECDSA stake registry");
        let ecdsa_stake_registry_impl =
            ECDSAStakeRegistry::deploy(self.eth.http_provider.clone(), core.delegation_manager)
                .await?;

        tracing::debug!("deploying Hello world registry");
        let hello_world_impl = HelloWorldServiceManager::deploy(
            self.eth.http_provider.clone(),
            core.avs_directory,
            proxies.ecdsa_stake_registry,
            core.rewards_coordinator,
            core.delegation_manager,
        )
        .await?;

        let upgrade_call = ECDSAStakeRegistry::initializeCall {
            _serviceManager: proxies.hello_world,
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
            .upgrade(proxies.hello_world, *hello_world_impl.address())
            .send()
            .await?
            .watch()
            .await?;

        let underlying_token = strategy.underlyingToken().call().await?._0;
        assert_ne!(underlying_token, Address::ZERO);
        tracing::debug!("underlying strategy token addr: {}", underlying_token);

        // Upgrade contracts
        Ok(HelloWorldClient {
            eth: self.eth,
            core,
            hello_world: HelloWorldDeployment {
                addresses: HelloWorldAddressesConfig {
                    proxy_admin: *proxies.admin.address(),
                    hello_world_service_manager: proxies.hello_world,
                    stake_registry: proxies.ecdsa_stake_registry,
                    token: setup.token,
                },
            },
        })
    }
}

struct Proxies {
    pub admin: ProxyAdminT,
    pub hello_world: Address,
    pub ecdsa_stake_registry: Address,
}

impl Proxies {
    pub async fn new(eth: &EthSigningClient) -> Result<Self> {
        let admin = ProxyAdmin::deploy(eth.http_provider.clone()).await?;

        tracing::debug!("Eigen core proxy admin: {}", admin.address());

        Ok(Self {
            ecdsa_stake_registry: setup_empty_proxy(eth, &admin).await?,
            hello_world: setup_empty_proxy(eth, &admin).await?,
            admin,
        })
    }
}
