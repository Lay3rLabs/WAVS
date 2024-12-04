use super::{
    config::CoreAVSAddresses,
    solidity_types::{
        delegation_manager::DelegationManager,
        misc::{
            AVSDirectory, EigenPod, EigenPodManager, PauserRegistry, RewardsCoordinator,
            StrategyBase, StrategyFactory, StrategyManager, UpgradeableBeacon,
        },
        proxy::{EmptyContract, ProxyAdmin, TransparentUpgradeableProxy},
        EmptyContractT, ProxyAdminT, TransparentProxyContractT,
    },
    EigenClient,
};
use crate::eth_client::EthSigningClient;
use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::Provider;
use alloy::sol_types::SolCall;
use anyhow::Result;

// TODO: read anvil config from: lib/eigenlayer-middleware/lib/eigenlayer-contracts/script/configs/local/deploy_from_scratch.anvil.config.json

impl EigenClient {
    pub async fn deploy_core_contracts(&self) -> Result<CoreAVSAddresses> {
        tracing::debug!("deploying proxies");

        let mut proxies = Proxies::new(&self.eth).await?;

        // sanity check - we own the ProxyAdmin
        debug_assert_eq!(proxies.admin.owner().call().await?._0, self.eth.address());

        tracing::debug!("deploying delegation manager");
        let delegation_manager_impl = DelegationManager::deploy(
            self.eth.http_provider.clone(),
            proxies.strategy_manager,
            Address::ZERO,
            proxies.eigen_pod_manager,
        )
        .await?;

        tracing::debug!("deploying avs directory");
        let avs_directory_impl =
            AVSDirectory::deploy(self.eth.http_provider.clone(), proxies.delegation_manager)
                .await?;

        tracing::debug!("deploying strategy manager");
        let strategy_manager_impl = StrategyManager::deploy(
            self.eth.http_provider.clone(),
            proxies.delegation_manager,
            proxies.eigen_pod_manager,
            Address::ZERO,
        )
        .await?;

        tracing::debug!("deploying strategy factory");
        let strategy_factory_impl =
            StrategyFactory::deploy(self.eth.http_provider.clone(), proxies.strategy_manager)
                .await?;

        let eth_deposit_addr = Address::ZERO;

        // if (block.chainid == 1) {
        //     ethPOSDeposit = 0x00000000219ab540356cBB839Cbe05303d7705Fa;
        // } else {
        //     // For non-mainnet chains, you might want to deploy a mock or read from a config
        //     // This assumes you have a similar config setup as in M2_Deploy_From_Scratch.s.sol
        //     /// TODO: Handle Eth pos
        // }

        tracing::debug!("deploying eigen pod manager");
        let eigen_pod_manager_impl = EigenPodManager::deploy(
            self.eth.http_provider.clone(),
            eth_deposit_addr,
            proxies.eigen_pod_beacon,
            proxies.strategy_manager,
            Address::ZERO,
            proxies.delegation_manager,
        )
        .await?;

        tracing::debug!("deploying rewards coordinator");
        let rewards_coordinator_impl = RewardsCoordinator::deploy(
            self.eth.http_provider.clone(),
            proxies.delegation_manager,
            proxies.strategy_manager,
            // TODO: Get actual values
            86400,
            86400,
            1,
            1,
            864000,
        )
        .await?;

        tracing::debug!("deploying eigen pod");
        let eigen_pod_impl = EigenPod::deploy(
            self.eth.http_provider.clone(),
            eth_deposit_addr,
            proxies.eigen_pod_manager,
            // TODO: Get actual genesis time
            1_564_000,
        )
        .await?;

        // Unused?
        //
        // tracing::debug!("deploying eigen beacon");
        // let eigen_pod_beacon_impl = UpgradeableBeacon::deploy(
        //     self.eth.http_provider.clone(),
        //     *eigen_pod_impl.address(),
        // )
        // .await?;

        tracing::debug!("deploying strategy base");
        let base_strategy_impl =
            StrategyBase::deploy(self.eth.http_provider.clone(), proxies.strategy_manager).await?;

        tracing::debug!("deploying pauser registry");
        let pauser_registry_impl = PauserRegistry::deploy(
            self.eth.http_provider.clone(),
            vec![],
            *proxies.admin.address(),
        )
        .await?;

        tracing::debug!("deploying upgradeable beacon");
        proxies.strategy_beacon = *UpgradeableBeacon::deploy(
            self.eth.http_provider.clone(),
            *base_strategy_impl.address(),
        )
        .await?
        .address();

        tracing::debug!("upgrading delegation manager");
        let upgrade_call = DelegationManager::initializeCall {
            initialOwner: *proxies.admin.address(),
            _pauserRegistry: *pauser_registry_impl.address(),
            initialPausedStatus: U256::ZERO,
            _minWithdrawalDelayBlocks: U256::ZERO,
            _withdrawalDelayBlocks: Vec::new(),
            _strategies: Vec::new(),
        };

        proxies
            .admin
            .upgradeAndCall(
                proxies.delegation_manager,
                *delegation_manager_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        tracing::debug!("upgrading strategy manager");
        // Upgrade strategy manager
        let upgrade_call = StrategyManager::initializeCall {
            initialOwner: *proxies.admin.address(),
            initialStrategyWhitelister: proxies.strategy_factory,
            _pauserRegistry: proxies.pauser_registry,
            initialPausedStatus: U256::ZERO,
        };

        proxies
            .admin
            .upgradeAndCall(
                proxies.strategy_manager,
                *strategy_manager_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        tracing::debug!("upgrading strategy factory");
        // Upgrade StrategyFactory
        let upgrade_call = StrategyFactory::initializeCall {
            _initialOwner: *proxies.admin.address(),
            _pauserRegistry: proxies.pauser_registry,
            _initialPausedStatus: U256::ZERO,
            _strategyBeacon: proxies.strategy_beacon,
        };

        proxies
            .admin
            .upgradeAndCall(
                proxies.strategy_factory,
                *strategy_factory_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        tracing::debug!("upgrading eigen pod manager");
        // Upgrade EigenPodManager
        let upgrade_call = EigenPodManager::initializeCall {
            initialOwner: *proxies.admin.address(),
            _pauserRegistry: proxies.pauser_registry,
            _initPausedStatus: U256::ZERO,
        };

        proxies
            .admin
            .upgradeAndCall(
                proxies.eigen_pod_manager,
                *eigen_pod_manager_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        tracing::debug!("upgrading avs directory");
        // Upgrade AVSDirectory
        let upgrade_call = AVSDirectory::initializeCall {
            initialOwner: *proxies.admin.address(),
            _pauserRegistry: proxies.pauser_registry,
            initialPausedStatus: U256::ZERO,
        };

        proxies
            .admin
            .upgradeAndCall(
                proxies.avs_directory,
                *avs_directory_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        tracing::debug!("upgrading rewards coordinator");
        // Upgrade RewardsCoordinator
        let upgrade_call = RewardsCoordinator::initializeCall {
            initialOwner: *proxies.admin.address(),
            _pauserRegistry: proxies.pauser_registry,
            initialPausedStatus: U256::ZERO,
            _rewardsUpdater: Address::ZERO,
            _activationDelay: 7200,
            _defaultSplitBips: 1000,
        };

        proxies
            .admin
            .upgradeAndCall(
                proxies.rewards_coordinator,
                *rewards_coordinator_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        // Upgrade EigenPod
        tracing::debug!("upgrading eigen pod");
        let upgrade_call = EigenPod::initializeCall {
            _podOwner: proxies.eigen_pod_manager,
        };

        proxies
            .admin
            .upgradeAndCall(
                proxies.eigen_pod_beacon,
                *eigen_pod_impl.address(),
                upgrade_call.abi_encode().into(),
            )
            .send()
            .await?
            .watch()
            .await?;

        tracing::debug!("Deployed eigen core");

        Ok(proxies.into())
    }
}

struct Proxies {
    pub admin: ProxyAdminT,
    pub delegation_manager: Address,
    pub avs_directory: Address,
    pub strategy_manager: Address,
    pub eigen_pod_manager: Address,
    pub rewards_coordinator: Address,
    pub eigen_pod_beacon: Address,
    pub pauser_registry: Address,
    pub strategy_factory: Address,
    pub strategy_beacon: Address,
}

impl From<Proxies> for CoreAVSAddresses {
    fn from(value: Proxies) -> Self {
        Self {
            proxy_admin: *value.admin.address(),
            delegation_manager: value.delegation_manager,
            avs_directory: value.avs_directory,
            strategy_manager: value.strategy_manager,
            eigen_pod_manager: value.eigen_pod_manager,
            rewards_coordinator: value.rewards_coordinator,
            eigen_pod_beacon: value.eigen_pod_beacon,
            pauser_registry: value.pauser_registry,
            strategy_factory: value.strategy_factory,
            strategy_beacon: value.strategy_beacon,
        }
    }
}

pub async fn setup_empty_proxy_all(
    eth: &EthSigningClient,
    proxy_admin: &ProxyAdminT,
) -> Result<(EmptyContractT, TransparentProxyContractT)> {
    let proxy_admin_address = *proxy_admin.address();

    let empty_contract = EmptyContract::deploy(eth.http_provider.clone()).await?;
    let empty_contract_address = *empty_contract.address();
    let proxy = TransparentUpgradeableProxy::deploy(
        eth.http_provider.clone(),
        empty_contract_address,
        proxy_admin_address,
        b"".into(),
    )
    .await?;

    #[cfg(debug_assertions)]
    {
        tracing::debug!("sanity checking admin...");
        // Sanity checks - ensure the proxy admin is set correctly
        // see TransparentUpgradeableProxy.sol: function admin()

        // 1. check by storage
        let admin_slot: FixedBytes<32> = alloy::hex::decode(
            "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103",
        )?
        .as_slice()
        .try_into()?;
        let admin_address = eth
            .http_provider
            .get_storage_at(*proxy.address(), admin_slot.into())
            .await?;
        let admin_address: Address = Address::from_slice(&admin_address.to_be_bytes::<32>()[12..]);
        assert_eq!(admin_address, proxy_admin_address);

        // 2. check by Calling via proxy_admin helper function (also loads via storage)
        let admin_address = proxy_admin.getProxyAdmin(*proxy.address()).call().await?._0;
        assert_eq!(admin_address, proxy_admin_address);

        // 3. check that we can use proxy admin to do admin stuff
        let _ = proxy_admin
            .changeProxyAdmin(*proxy.address(), admin_address)
            .send()
            .await?
            .watch()
            .await?;
    }

    Ok((empty_contract, proxy))
}

pub async fn setup_empty_proxy(
    eth: &EthSigningClient,
    proxy_admin: &ProxyAdminT,
) -> Result<Address> {
    let (_, proxy) = setup_empty_proxy_all(eth, proxy_admin).await?;
    Ok(*proxy.address())
}

impl Proxies {
    pub async fn new(eth: &EthSigningClient) -> Result<Self> {
        let admin = ProxyAdmin::deploy(eth.http_provider.clone()).await?;

        tracing::debug!("Eigen core proxy admin: {}", *admin.address());
        let (_, delegation_manager_proxy) = setup_empty_proxy_all(eth, &admin).await?;

        Ok(Self {
            delegation_manager: *delegation_manager_proxy.address(),
            avs_directory: setup_empty_proxy(eth, &admin).await?,
            strategy_manager: setup_empty_proxy(eth, &admin).await?,
            eigen_pod_manager: setup_empty_proxy(eth, &admin).await?,
            rewards_coordinator: setup_empty_proxy(eth, &admin).await?,
            eigen_pod_beacon: setup_empty_proxy(eth, &admin).await?,
            pauser_registry: setup_empty_proxy(eth, &admin).await?,
            strategy_factory: setup_empty_proxy(eth, &admin).await?,
            // Initialized later
            strategy_beacon: Address::ZERO,
            admin,
        })
    }
}
