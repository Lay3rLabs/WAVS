use crate::eigen_client::solidity_types::BoxSigningProvider;
use alloy::{sol, transports::BoxTransport};
use layer_service_manager::LayerServiceManager::LayerServiceManagerInstance;

pub mod stake_registry {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ECDSAStakeRegistry,
        "../../contracts/solidity/abi/ECDSAStakeRegistry.sol/ECDSAStakeRegistry.json"
    );
}

pub mod token {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerToken,
        "../../contracts/solidity/abi/LayerToken.sol/LayerToken.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        IStrategy,
        "../../contracts/solidity/abi/IStrategy.sol/IStrategy.json"
    );
}

pub mod layer_service_manager {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerServiceManager,
        "../../contracts/solidity/abi/LayerServiceManager.sol/LayerServiceManager.json"
    );
}

pub type LayerServiceManagerT = LayerServiceManagerInstance<BoxTransport, BoxSigningProvider>;
