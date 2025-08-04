#[cfg(not(feature = "solidity-rpc"))]
pub use not_rpc::*;

#[cfg(feature = "solidity-rpc")]
pub use rpc::*;

#[cfg(not(feature = "solidity-rpc"))]
mod not_rpc {
    mod service_manager {
        alloy_sol_macro::sol!(
            #[allow(missing_docs)]
            #[derive(Debug)]
            IWavsServiceManager,
            "./src/contracts/solidity/abi/IWavsServiceManager.sol/IWavsServiceManager.json"
        );
    }

    mod service_handler {
        alloy_sol_macro::sol!(
            #[allow(missing_docs)]
            #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
            IWavsServiceHandler,
            "./src/contracts/solidity/abi/IWavsServiceHandler.sol/IWavsServiceHandler.json"
        );
    }

    pub use service_manager::{
        IWavsServiceManager,
        // yup, the service handler interface as seen by the service manager is a different service handler interface
        // even though it's literally a direct import of the same file
        IWavsServiceHandler::Envelope as ServiceManagerEnvelope,
        IWavsServiceHandler::SignatureData as ServiceManagerSignatureData,
    };

    pub type ServiceManagerError = IWavsServiceManager::IWavsServiceManagerErrors;

    pub use service_handler::{
        IWavsServiceHandler, IWavsServiceHandler::Envelope, IWavsServiceHandler::SignatureData,
    };
}


#[cfg(feature = "solidity-rpc")]
mod rpc {
    mod service_manager {
        alloy_sol_macro::sol!(
            #[allow(missing_docs)]
            #[sol(rpc)]
            #[derive(Debug)]
            IWavsServiceManager,
            "./src/contracts/solidity/abi/IWavsServiceManager.sol/IWavsServiceManager.json"
        );
    }

    mod service_handler {

        alloy_sol_macro::sol!(
            #[allow(missing_docs)]
            #[sol(rpc)]
            #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
            IWavsServiceHandler,
            "./src/contracts/solidity/abi/IWavsServiceHandler.sol/IWavsServiceHandler.json"
        );
    }

    pub type ServiceManagerError = IWavsServiceManager::IWavsServiceManagerErrors;

    pub use service_handler::{
        IWavsServiceHandler, IWavsServiceHandler::Envelope, IWavsServiceHandler::SignatureData,
    };

    pub use service_manager::{
        IWavsServiceManager,
        // yup, the service handler interface as seen by the service manager is a different service handler interface
        // even though it's literally a direct import of the same file
        IWavsServiceHandler::Envelope as ServiceManagerEnvelope,
        IWavsServiceHandler::SignatureData as ServiceManagerSignatureData,
    };

    pub fn decode_service_manager_error(err: alloy_contract::Error) -> Option<ServiceManagerError> {
        err.as_decoded_interface_error::<ServiceManagerError>()
    }

    use alloy_provider::DynProvider;

    pub type IWavsServiceHandlerSigningT =
        service_handler::IWavsServiceHandler::IWavsServiceHandlerInstance<DynProvider>;

    pub type IWavsServiceHandlerQueryT =
        service_handler::IWavsServiceHandler::IWavsServiceHandlerInstance<DynProvider>;

    pub type IWavsServiceManagerSigningT =
        service_manager::IWavsServiceManager::IWavsServiceManagerInstance<DynProvider>;

    pub type IWavsServiceManagerQueryT =
        service_manager::IWavsServiceManager::IWavsServiceManagerInstance<DynProvider>;
}
