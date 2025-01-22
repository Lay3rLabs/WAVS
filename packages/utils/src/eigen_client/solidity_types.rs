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
    transports::{
        http::{Client, Http},
        BoxTransport,
    },
};

pub mod delegation_manager {
    use super::*;

    sol!(
        #[sol(rpc)]
        DelegationManager,
        "../../sdk/solidity/contracts/abi/DelegationManager.sol/DelegationManager.json"
    );
}

pub mod proxy {
    use super::*;

    sol!(
        #[sol(rpc)]
        EmptyContract,
        "../../sdk/solidity/contracts/abi/EmptyContract.sol/EmptyContract.json"
    );

    sol!(
        #[sol(rpc)]
        TransparentUpgradeableProxy,
        "../../sdk/solidity/contracts/abi/TransparentUpgradeableProxy.sol/TransparentUpgradeableProxy.json"
    );

    sol!(
        #[sol(rpc)]
        ProxyAdmin,
        "../../sdk/solidity/contracts/abi/ProxyAdmin.sol/ProxyAdmin.json"
    );
}

pub mod misc {
    use super::*;

    sol!(
        #[sol(rpc)]
        PauserRegistry,
        "../../sdk/solidity/contracts/abi/PauserRegistry.sol/PauserRegistry.json"
    );

    sol!(
        #[sol(rpc)]
        AVSDirectory,
        "../../sdk/solidity/contracts/abi/AVSDirectory.sol/AVSDirectory.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyManager,
        "../../sdk/solidity/contracts/abi/StrategyManager.sol/StrategyManager.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyFactory,
        "../../sdk/solidity/contracts/abi/StrategyFactory.sol/StrategyFactory.json"
    );

    sol!(
        #[sol(rpc)]
        EigenPodManager,
        "../../sdk/solidity/contracts/abi/EigenPodManager.sol/EigenPodManager.json"
    );

    sol!(
        #[sol(rpc)]
        RewardsCoordinator,
        "../../sdk/solidity/contracts/abi/RewardsCoordinator.sol/RewardsCoordinator.json"
    );

    sol!(
        #[sol(rpc)]
        EigenPod,
        "../../sdk/solidity/contracts/abi/EigenPod.sol/EigenPod.json"
    );

    sol!(
        #[sol(rpc)]
        UpgradeableBeacon,
        "../../sdk/solidity/contracts/abi/UpgradeableBeacon.sol/UpgradeableBeacon.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyBase,
        "../../sdk/solidity/contracts/abi/StrategyBase.sol/StrategyBase.json"
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
    proxy::EmptyContract::EmptyContractInstance<BoxTransport, BoxSigningProvider>;

pub type TransparentProxyContractT =
    proxy::TransparentUpgradeableProxy::TransparentUpgradeableProxyInstance<
        BoxTransport,
        BoxSigningProvider,
    >;

pub type ProxyAdminT = proxy::ProxyAdmin::ProxyAdminInstance<BoxTransport, BoxSigningProvider>;

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

pub type BoxSigningProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<BoxTransport>,
    BoxTransport,
    Ethereum,
>;
