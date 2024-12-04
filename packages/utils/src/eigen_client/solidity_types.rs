#![allow(clippy::too_many_arguments)]
use alloy::sol;

pub mod delegation_manager {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        DelegationManager,
        "../../out/DelegationManager.sol/DelegationManager.json"
    );
}

pub mod proxy {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        EmptyContract,
        "../../out/EmptyContract.sol/EmptyContract.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        TransparentUpgradeableProxy,
        "../../out/TransparentUpgradeableProxy.sol/TransparentUpgradeableProxy.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ProxyAdmin,
        "../../out/ProxyAdmin.sol/ProxyAdmin.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        UpgradeableProxyLib,
        "../../out/UpgradeableProxyLib.sol/UpgradeableProxyLib.json"
    );
}

pub mod misc {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        PauserRegistry,
        "../../out/PauserRegistry.sol/PauserRegistry.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        AVSDirectory,
        "../../out/AVSDirectory.sol/AVSDirectory.json"
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
}
