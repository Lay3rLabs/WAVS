use alloy::sol;
use layer_service_manager::WavsServiceManager::WavsServiceManagerInstance;

use crate::eth_client::SigningProvider;

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
        WavsServiceManager,
        "../../contracts/solidity/abi/WavsServiceManager.sol/WavsServiceManager.json"
    );
}

pub mod layer_service_aggregator {
    use super::*;
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        WavsServiceAggregator,
        "../../contracts/solidity/abi/WavsServiceAggregator.sol/WavsServiceAggregator.json"
    );
}

pub mod layer_service_handler {
    use super::*;
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        IWavsServiceHandler,
        "../../contracts/solidity/abi/IWavsServiceHandler.sol/IWavsServiceHandler.json"
    );
}

pub type WavsServiceManagerT = WavsServiceManagerInstance<(), SigningProvider>;
pub type IWavsServiceHandlerT =
    layer_service_handler::IWavsServiceHandler::IWavsServiceHandlerInstance<(), SigningProvider>;
