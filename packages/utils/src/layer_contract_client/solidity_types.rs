use crate::eigen_client::solidity_types::BoxSigningProvider;
use alloy::{sol, transports::BoxTransport};
use layer_service_manager::LayerServiceManager::LayerServiceManagerInstance;
use layer_trigger::LayerTrigger::LayerTriggerInstance;

pub mod stake_registry {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ECDSAStakeRegistry,
        "../../contracts/abi/ECDSAStakeRegistry.sol/ECDSAStakeRegistry.json"
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

pub mod layer_service_manager {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerServiceManager,
        "../../contracts/abi/LayerServiceManager.sol/LayerServiceManager.json"
    );
}

pub mod layer_trigger {
    use super::*;
    pub use ILayerTrigger::TriggerResponse;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LayerTrigger,
        "../../contracts/abi/LayerTrigger.sol/LayerTrigger.json"
    );
}

pub type LayerServiceManagerT = LayerServiceManagerInstance<BoxTransport, BoxSigningProvider>;

pub type LayerTriggerT = LayerTriggerInstance<BoxTransport, BoxSigningProvider>;
