use alloy::sol;

pub mod stake_registry {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ECDSAStakeRegistry,
        "../../out/ECDSAStakeRegistry.sol/ECDSAStakeRegistry.json"
    );
}

pub mod hello_world {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        HelloWorldServiceManager,
        "../../out/HelloWorldServiceManager.sol/HelloWorldServiceManager.json"
    );
}

pub mod token {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerToken,
        "../../out/LayerToken.sol/LayerToken.json"
    );
}
