use alloy::sol;

pub mod stake_registry {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ECDSAStakeRegistry,
        "../../contracts/abi/ECDSAStakeRegistry.sol/ECDSAStakeRegistry.json"
    );
}

pub mod hello_world {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        HelloWorldServiceManager,
        "../../contracts/abi/HelloWorldServiceManager.sol/HelloWorldServiceManager.json"
    );
}

pub mod token {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerToken,
        "../../contracts/abi/LayerToken.sol/LayerToken.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        IStrategy,
        "../../contracts/abi/IStrategy.sol/IStrategy.json"
    );
}
