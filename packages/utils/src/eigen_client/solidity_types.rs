#![allow(clippy::too_many_arguments)]
#![allow(missing_docs, non_snake_case)]

use alloy::sol;

use crate::eth_client::SigningProvider;

pub mod delegation_manager {
    use super::*;

    sol!(
        #[sol(rpc)]
        DelegationManager,
        "../../contracts/solidity/abi/DelegationManager.sol/DelegationManager.json"
    );
}

pub mod proxy {
    use super::*;

    sol!(
        #[sol(rpc)]
        EmptyContract,
        "../../contracts/solidity/abi/EmptyContract.sol/EmptyContract.json"
    );

    sol!(
        #[sol(rpc)]
        TransparentUpgradeableProxy,
        "../../contracts/solidity/abi/TransparentUpgradeableProxy.sol/TransparentUpgradeableProxy.json"
    );

    sol!(
        #[sol(rpc)]
        ProxyAdmin,
        "../../contracts/solidity/abi/ProxyAdmin.sol/ProxyAdmin.json"
    );
}

pub mod misc {
    use super::*;

    sol!(
        #[sol(rpc)]
        PauserRegistry,
        "../../contracts/solidity/abi/PauserRegistry.sol/PauserRegistry.json"
    );

    sol!(
        #[sol(rpc)]
        AVSDirectory,
        "../../contracts/solidity/abi/AVSDirectory.sol/AVSDirectory.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyManager,
        "../../contracts/solidity/abi/StrategyManager.sol/StrategyManager.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyFactory,
        "../../contracts/solidity/abi/StrategyFactory.sol/StrategyFactory.json"
    );

    sol!(
        #[sol(rpc)]
        EigenPodManager,
        "../../contracts/solidity/abi/EigenPodManager.sol/EigenPodManager.json"
    );

    sol!(
        #[sol(rpc)]
        RewardsCoordinator,
        "../../contracts/solidity/abi/RewardsCoordinator.sol/RewardsCoordinator.json"
    );

    sol!(
        #[sol(rpc)]
        EigenPod,
        "../../contracts/solidity/abi/EigenPod.sol/EigenPod.json"
    );

    sol!(
        #[sol(rpc)]
        UpgradeableBeacon,
        "../../contracts/solidity/abi/UpgradeableBeacon.sol/UpgradeableBeacon.json"
    );

    sol!(
        #[sol(rpc)]
        StrategyBase,
        "../../contracts/solidity/abi/StrategyBase.sol/StrategyBase.json"
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

pub type EmptyContractT = proxy::EmptyContract::EmptyContractInstance<(), SigningProvider>;

pub type TransparentProxyContractT =
    proxy::TransparentUpgradeableProxy::TransparentUpgradeableProxyInstance<(), SigningProvider>;

pub type ProxyAdminT = proxy::ProxyAdmin::ProxyAdminInstance<(), SigningProvider>;
