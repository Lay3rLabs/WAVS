#![allow(clippy::too_many_arguments)]
#![allow(missing_docs, non_snake_case)]

use alloy::{
    network::{Ethereum, EthereumWallet},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, RootProvider,
    },
    pubsub::PubSubFrontend,
    sol,
    transports::http::{Client, Http},
};

pub mod delegation_manager {
    use super::*;

    sol!(
        #[sol(rpc)]
        DelegationManager,
        "../../contracts/abi/DelegationManager.sol/DelegationManager.json"
    );
}

pub mod proxy {
    use super::*;

    sol!(
        #[sol(rpc)]
        EmptyContract,
        "../../contracts/abi/EmptyContract.sol/EmptyContract.json"
    );

    sol!(
        #[sol(rpc)]
        TransparentUpgradeableProxy,
        "../../contracts/abi/TransparentUpgradeableProxy.sol/TransparentUpgradeableProxy.json"
    );

    sol!(
        #[sol(rpc)]
        ProxyAdmin,
        "../../contracts/abi/ProxyAdmin.sol/ProxyAdmin.json"
    );
}

pub mod misc {
    use super::*;

    sol!(
        #[sol(rpc)]
        PauserRegistry,
        "../../contracts/abi/PauserRegistry.sol/PauserRegistry.json"
    );

    sol!(
        #[sol(rpc)]
        AVSDirectory,
        "../../contracts/abi/AVSDirectory.sol/AVSDirectory.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyManager,
        "../../contracts/abi/StrategyManager.sol/StrategyManager.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyFactory,
        "../../contracts/abi/StrategyFactory.sol/StrategyFactory.json"
    );

    sol!(
        #[sol(rpc)]
        EigenPodManager,
        "../../contracts/abi/EigenPodManager.sol/EigenPodManager.json"
    );

    sol!(
        #[sol(rpc)]
        RewardsCoordinator,
        "../../contracts/abi/RewardsCoordinator.sol/RewardsCoordinator.json"
    );

    sol!(
        #[sol(rpc)]
        EigenPod,
        "../../contracts/abi/EigenPod.sol/EigenPod.json"
    );

    sol!(
        #[sol(rpc)]
        UpgradeableBeacon,
        "../../contracts/abi/UpgradeableBeacon.sol/UpgradeableBeacon.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyBase,
        "../../contracts/abi/StrategyBase.sol/StrategyBase.json"
    );

    // It's enum, but alloy didn't generate helpers for it
    impl IAVSDirectory::OperatorAVSRegistrationStatus {
        pub fn UNREGISTERED() -> Self {
            Self::from(0u8)
        }

        pub fn REGISTERED() -> Self {
            Self::from(1u8)
        }
    }
}

pub type EmptyContractT =
    proxy::EmptyContract::EmptyContractInstance<Http<Client>, HttpSigningProvider>;

pub type TransparentProxyContractT =
    proxy::TransparentUpgradeableProxy::TransparentUpgradeableProxyInstance<
        Http<Client>,
        HttpSigningProvider,
    >;

pub type ProxyAdminT = proxy::ProxyAdmin::ProxyAdminInstance<Http<Client>, HttpSigningProvider>;

pub type WsSigningProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<PubSubFrontend>,
    PubSubFrontend,
    Ethereum,
>;

pub type HttpSigningProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<Http<Client>>,
    Http<Client>,
    Ethereum,
>;
