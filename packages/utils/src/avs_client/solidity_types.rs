use crate::eigen_client::solidity_types::BoxSigningProvider;
use alloy::{sol, transports::BoxTransport};
use layer_service_manager::LayerServiceManager::LayerServiceManagerInstance;

pub mod stake_registry {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ECDSAStakeRegistry,
        "../../sdk/solidity/contracts/abi/ECDSAStakeRegistry.sol/ECDSAStakeRegistry.json"
    );
}

pub mod token {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerToken,
        "../../sdk/solidity/contracts/abi/LayerToken.sol/LayerToken.json"
    );

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        IStrategy,
        "../../sdk/solidity/contracts/abi/IStrategy.sol/IStrategy.json"
    );
}

pub mod layer_service_manager {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerServiceManager,
        "../../sdk/solidity/contracts/abi/LayerServiceManager.sol/LayerServiceManager.json"
    );
}

pub mod layer_trigger {
    use super::*;
    pub use ILayerTrigger::LayerTriggerEvent;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ILayerTrigger,
        "../../sdk/solidity/contracts/abi/ILayerTrigger.sol/ILayerTrigger.json"
    );
}

pub type LayerServiceManagerT = LayerServiceManagerInstance<BoxTransport, BoxSigningProvider>;
