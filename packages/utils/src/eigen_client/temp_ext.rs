use std::{ops::Add, sync::Arc};

use crate::eigen_client::config::EigenClientConfig;
use crate::{error::EthClientError, eth_client::EthSigningClient};
use alloy::dyn_abi::abi;
use alloy::primitives::{FixedBytes, U160};
use alloy::sol_types::SolCall;
use alloy::{
    network::{Ethereum, EthereumWallet},
    primitives::{
        Address,
        keccak256
    },
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, ProviderBuilder, RootProvider, WsConnect,
    },
    pubsub::PubSubFrontend,
    signers::{
        k256::ecdsa::SigningKey,
        local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
    },
    transports::http::{Client, Http},
};
use ProxyAdmin::ProxyAdminInstance;
//use eigen_utils::delegationmanager::{DelegationManager::{self}, IDelegationManager::OperatorDetails};
use super::*;
use super::EmptyContract::EmptyContractInstance;
use super::TransparentUpgradeableProxy::TransparentUpgradeableProxyInstance;
use anyhow::{Context, Result};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    AvsDirectory,
    "../../out/AvsDirectory.sol/AvsDirectory.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    StrategyManager,
    "../../out/StrategyManager.sol/StrategyManager.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    StrategyFactory,
    "../../out/StrategyFactory.sol/StrategyFactory.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EigenPodManager,
    "../../out/EigenPodManager.sol/EigenPodManager.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    RewardsCoordinator,
    "../../out/RewardsCoordinator.sol/RewardsCoordinator.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EigenPod,
    "../../out/EigenPod.sol/EigenPod.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    UpgradeableBeacon,
    "../../out/UpgradeableBeacon.sol/UpgradeableBeacon.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    StrategyBase,
    "../../out/StrategyBase.sol/StrategyBase.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    UpgradeableProxyLib,
    "../../out/UpgradeableProxyLib.sol/UpgradeableProxyLib.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    Vm,
    "../../out/Vm.sol/Vm.json"
);

impl EigenClient {
    pub async fn deploy_core_contracts(&self) -> Result<()> {
        let proxies = ProxyAddresses::new(&self.eth).await?;

        let delegation_manager_impl = DelegationManager::deploy(
            self.eth.http_provider.clone(),
            proxies.strategy_manager.clone(),
            Address::ZERO,
            proxies.eigen_pod_manager,
        )
        .await?;


        // FIXME: why is owner 0000000?
        // according to OwnableUpgradeable (the base contract providing owner()),
        // "By default, the owner account will be the one that deploys the contract"
        // so... why is `deploy()` not setting it to the wallet address??
        println!("wallet address: {}", self.eth.address());
        println!("owner before: {}", delegation_manager_impl.owner().call().await?._0);
        let resp = delegation_manager_impl.transferOwnership(proxies.admin.address().clone()).send().await?;
        println!("owner after: {}", delegation_manager_impl.owner().call().await?._0);




        let avs_directory_impl = AvsDirectory::deploy(self.eth.http_provider.clone(), proxies.delegation_manager.clone()).await?;    

        let strategy_manager_impl = StrategyManager::deploy(self.eth.http_provider.clone(), proxies.delegation_manager.clone(), proxies.eigen_pod_manager.clone(), Address::ZERO).await?;

        let strategy_factory_impl = StrategyFactory::deploy(self.eth.http_provider.clone(), proxies.strategy_manager.clone()).await?;

        let eth_deposit_addr = Address::ZERO;

        // if (block.chainid == 1) {
        //     ethPOSDeposit = 0x00000000219ab540356cBB839Cbe05303d7705Fa;
        // } else {
        //     // For non-mainnet chains, you might want to deploy a mock or read from a config
        //     // This assumes you have a similar config setup as in M2_Deploy_From_Scratch.s.sol
        //     /// TODO: Handle Eth pos
        // }


        let eigen_pod_manager_impl = EigenPodManager::deploy(
            self.eth.http_provider.clone(), 
            eth_deposit_addr, 
            proxies.eigen_pod_beacon.clone(), 
            proxies.strategy_manager.clone(), 
            Address::ZERO, 
            proxies.delegation_manager.clone(),
        ).await?;

        let rewards_coordinator_impl = RewardsCoordinator::deploy(
            self.eth.http_provider.clone(),
            proxies.delegation_manager.clone(),
            proxies.strategy_manager.clone(),
            /// TODO: Get actual values
            86400,
            86400,
            1,
            1,
            864000 
        ).await?;

        let eigen_pod_impl = EigenPod::deploy(
            self.eth.http_provider.clone(),
            eth_deposit_addr,
            proxies.eigen_pod_manager.clone(),
            // TODO: Get actual genesis time
            1_564_000
        ).await?;

        let eigen_pod_beacon_impl = UpgradeableBeacon::deploy(
            self.eth.http_provider.clone(),
            eigen_pod_impl.address().clone()
        ).await?;

        let base_strategy_impl = StrategyBase::deploy(
            self.eth.http_provider.clone(),
            proxies.strategy_manager.clone(),
        ).await?;

        let pauser_registry_impl = PauserRegistry::deploy(
            self.eth.http_provider.clone(),
            vec![],
            proxies.admin.address().clone()
        ).await?;

        let strategy_beacon_impl = UpgradeableBeacon::deploy(
            self.eth.http_provider.clone(),
            base_strategy_impl.address().clone()
        ).await?;

        // Upgrade Delegation Manager
        let upgrade_call = DelegationManager::initializeCall{
            initialOwner: proxies.admin.address().clone(),
            _pauserRegistry: pauser_registry_impl.address().clone(),
            initialPausedStatus: U256::ZERO,
            _minWithdrawalDelayBlocks: U256::ZERO,
            _withdrawalDelayBlocks: Vec::new(), 
            _strategies: Vec::new()
        };

        println!("delegation_manager_impl: {}", delegation_manager_impl.owner().call().await?._0);

        // let admin_slot:FixedBytes<32> = alloy::hex::decode("0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103")?.as_slice().try_into()?;
        // let proxy_admin:[u8;32] = self.eth.http_provider.get_storage_at(proxies.delegation_manager.clone(), admin_slot.into()).await?
        //     .to_be_bytes();
        // let proxy_admin = &proxy_admin[12..];
        // // println!("BP 6");
        // let proxy_admin = Address::from_slice(proxy_admin);
        // println!("proxy_admin from storage: {}", proxy_admin);

        proxies.admin.upgradeAndCall(proxies.delegation_manager, delegation_manager_impl.address().clone(), upgrade_call.abi_encode().into())
            .call()
            .await?;

        // UPgrade strategy manager
        // let upgrade_call = StrategyManager::initializeCall{
        //     _delegationManager: proxies.delegation_manager.clone(),
        //     _eigenPodManager: proxies.eigen_pod_manager.clone(),
        //     _strategyBeacon: strategy_beacon_impl.address().clone(),
        //     _strategies: Vec::new()
        // };


        println!("SO FAR SO GOOD!!");


        Ok(())
    }
}

fn vm_address() -> Address {
    // Step 1: Compute the Keccak256 hash of "hevm cheat code"
    let input = b"hevm cheat code";
    let hash = keccak256(input); // This produces a [u8; 32] array

    // Step 2: Convert the hash to U256
    let hash_u256 = U256::from_be_slice(hash.as_slice());

    // Step 3: Take the lower 160 bits (20 bytes) of the hash
    // Create an Address by taking the last 20 bytes
    let address_bytes = &hash[12..32]; // Bytes from index 12 to 31 inclusive
    Address::from_slice(address_bytes)

}

struct ProxyAddresses {
    pub admin: ProxyAdminInstance<Http<Client>, FillProvider<JoinFill<JoinFill<Identity, JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>>, WalletFiller<EthereumWallet>>, RootProvider<Http<Client>>, Http<Client>, Ethereum>>,
    pub delegation_manager: Address,
    pub avs_directory: Address,
    pub strategy_manager: Address,
    pub eigen_pod_manager: Address,
    pub rewards_coordinator: Address,
    pub eigen_pod_beacon: Address,
    pub pauser_registery: Address,
    pub strategy_factory: Address,
}

type EmptyContractT = EmptyContractInstance<Http<Client>, FillProvider<JoinFill<JoinFill<Identity, JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>>, WalletFiller<EthereumWallet>>, RootProvider<Http<Client>>, Http<Client>, Ethereum>>; 
type TransparentProxyContractT =TransparentUpgradeableProxyInstance<Http<Client>, FillProvider<JoinFill<JoinFill<Identity, JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>>, WalletFiller<EthereumWallet>>, RootProvider<Http<Client>>, Http<Client>, Ethereum>>;
impl ProxyAddresses {
    pub async fn new(eth: &EthSigningClient) -> Result<Self> {
        async fn setup_empty_proxy_all(
            eth: &EthSigningClient,
            proxy_admin: Address,
        ) -> Result<(EmptyContractT, TransparentProxyContractT)> {
            let empty_contract = EmptyContract::deploy(eth.http_provider.clone()).await?;
            let empty_contract_address = empty_contract.address().clone();
            let proxy = TransparentUpgradeableProxy::deploy(
                eth.http_provider.clone(),
                empty_contract_address,
                proxy_admin,
                b"".into(),
            )
            .await?;

            Ok((empty_contract, proxy))
        }

        async fn setup_empty_proxy(
            eth: &EthSigningClient,
            proxy_admin: Address,
        ) -> Result<Address> {
            let (empty_contract, proxy) = setup_empty_proxy_all(eth, proxy_admin).await?;
            Ok(proxy.address().clone())
        }

        let admin = ProxyAdmin::deploy(eth.http_provider.clone()).await?;

        println!("proxy admin: {}", admin.address().clone());
        let (delegation_manager_empty, delegation_manager_proxy) = setup_empty_proxy_all(eth, admin.address().clone()).await?;

        //println!("delegation_manager_proxy admin: {}", delegation_manager_proxy.admin().call().await?.admin_);

        Ok(Self {
            delegation_manager: delegation_manager_proxy.address().clone(),
            avs_directory: setup_empty_proxy(eth, admin.address().clone()).await?,
            strategy_manager: setup_empty_proxy(eth, admin.address().clone()).await?,
            eigen_pod_manager: setup_empty_proxy(eth, admin.address().clone()).await?,
            rewards_coordinator: setup_empty_proxy(eth, admin.address().clone()).await?,
            eigen_pod_beacon: setup_empty_proxy(eth, admin.address().clone()).await?,
            pauser_registery: setup_empty_proxy(eth, admin.address().clone()).await?,
            strategy_factory: setup_empty_proxy(eth, admin.address().clone()).await?,
            admin,
        })
    }
}
