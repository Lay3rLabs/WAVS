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

pub use service_handler::{IWavsServiceHandler, IWavsServiceHandler::Envelope};
// SignatureData is re-exported under alias to avoid collision with the signing::SignatureData enum.
// The raw Alloy type is accessible as crate::solidity_types::SignatureData.
pub(crate) use service_handler::IWavsServiceHandler::SignatureData;
pub use service_manager::IWavsServiceManager;
// yup, the service handler interface as seen by the service manager is a different service handler interface
// even though it's literally a direct import of the same file
pub use service_manager::{
    IWavsServiceHandler::Envelope as ServiceManagerEnvelope,
    IWavsServiceHandler::SignatureData as ServiceManagerSignatureData,
};
