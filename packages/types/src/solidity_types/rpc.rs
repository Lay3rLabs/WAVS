use alloy_provider::DynProvider;

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

pub use service_handler::{
    IWavsServiceHandler, IWavsServiceHandler::Envelope, IWavsServiceHandler::SignatureData,
};
pub use service_manager::IWavsServiceManager;
// yup, the service handler interface as seen by the service manager is a different service handler interface
// even though it's literally a direct import of the same file
pub use service_manager::{
    IWavsServiceHandler::Envelope as ServiceManagerEnvelope,
    IWavsServiceHandler::SignatureData as ServiceManagerSignatureData,
};

pub type IWavsServiceHandlerSigningT =
    IWavsServiceHandler::IWavsServiceHandlerInstance<DynProvider>;

pub type IWavsServiceHandlerQueryT = IWavsServiceHandler::IWavsServiceHandlerInstance<DynProvider>;

pub type IWavsServiceManagerSigningT =
    IWavsServiceManager::IWavsServiceManagerInstance<DynProvider>;

pub type IWavsServiceManagerQueryT = IWavsServiceManager::IWavsServiceManagerInstance<DynProvider>;

pub type ServiceManagerError = IWavsServiceManager::IWavsServiceManagerErrors;

pub fn decode_service_manager_error(err: alloy_contract::Error) -> Option<ServiceManagerError> {
    err.as_decoded_interface_error::<ServiceManagerError>()
}
