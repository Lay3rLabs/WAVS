#![allow(clippy::too_many_arguments)]
use alloy::sol;

pub mod delegation_manager {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        DelegationManager,
        "../../contracts/abi/DelegationManager.sol/DelegationManager.json"
    );
}

pub mod proxy {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        EmptyContract,
        "../../contracts/abi/EmptyContract.sol/EmptyContract.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        TransparentUpgradeableProxy,
        "../../contracts/abi/TransparentUpgradeableProxy.sol/TransparentUpgradeableProxy.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ProxyAdmin,
        "../../contracts/abi/ProxyAdmin.sol/ProxyAdmin.json"
    );
}

pub mod misc {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        PauserRegistry,
        "../../contracts/abi/PauserRegistry.sol/PauserRegistry.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        AVSDirectory,
        "../../contracts/abi/AVSDirectory.sol/AVSDirectory.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        StrategyManager,
        "../../contracts/abi/StrategyManager.sol/StrategyManager.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        StrategyFactory,
        "../../contracts/abi/StrategyFactory.sol/StrategyFactory.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        EigenPodManager,
        "../../contracts/abi/EigenPodManager.sol/EigenPodManager.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        RewardsCoordinator,
        "../../contracts/abi/RewardsCoordinator.sol/RewardsCoordinator.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        EigenPod,
        "../../contracts/abi/EigenPod.sol/EigenPod.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        UpgradeableBeacon,
        "../../contracts/abi/UpgradeableBeacon.sol/UpgradeableBeacon.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        StrategyBase,
        "../../contracts/abi/StrategyBase.sol/StrategyBase.json"
    );
}
