mod service_manager {
    use alloy::sol;

    sol!(
        #[allow(missing_docs)]
        #[cfg(feature = "solidity-rpc")]
        #[sol(rpc)]
        IWavsServiceManager,
        "../../contracts/solidity/abi/IWavsServiceManager.sol/IWavsServiceManager.json"
    );
}

mod service_handler {
    use alloy::sol;

    sol!(
        #[allow(missing_docs)]
        #[cfg(feature = "solidity-rpc")]
        #[sol(rpc)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IWavsServiceHandler,
        "../../contracts/solidity/abi/IWavsServiceHandler.sol/IWavsServiceHandler.json"
    );
}

pub use service_handler::{
    IWavsServiceHandler, IWavsServiceHandler::Envelope, IWavsServiceHandler::SignatureData,
};
pub use service_manager::IWavsServiceManager;

#[cfg(feature = "solidity-rpc")]
mod rpc {
    use alloy::providers::DynProvider;

    pub type IWavsServiceHandlerSigningT =
        super::service_handler::IWavsServiceHandler::IWavsServiceHandlerInstance<(), DynProvider>;

    pub type IWavsServiceHandlerQueryT =
        super::service_handler::IWavsServiceHandler::IWavsServiceHandlerInstance<(), DynProvider>;

    pub type IWavsServiceManagerSigningT =
        super::service_manager::IWavsServiceManager::IWavsServiceManagerInstance<(), DynProvider>;

    pub type IWavsServiceManagerQueryT =
        super::service_manager::IWavsServiceManager::IWavsServiceManagerInstance<(), DynProvider>;
}

#[cfg(feature = "solidity-rpc")]
pub use rpc::*;
