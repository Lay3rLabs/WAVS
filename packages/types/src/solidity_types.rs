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
        #[derive(serde::Deserialize, serde::Serialize, Debug)]
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
    pub type QueryProvider = FillProvider<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        RootProvider,
        Ethereum,
    >;
    pub type SigningProvider = FillProvider<
        JoinFill<
            JoinFill<
                Identity,
                JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
            >,
            WalletFiller<EthereumWallet>,
        >,
        RootProvider,
        Ethereum,
    >;

    use alloy::providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        network::{Ethereum, EthereumWallet},
        Identity, RootProvider,
    };

    pub type IWavsServiceHandlerSigningT =
        super::service_handler::IWavsServiceHandler::IWavsServiceHandlerInstance<
            (),
            SigningProvider,
        >;

    pub type IWavsServiceHandlerQueryT =
        super::service_handler::IWavsServiceHandler::IWavsServiceHandlerInstance<(), QueryProvider>;

    pub type IWavsServiceManagerSigningT =
        super::service_manager::IWavsServiceManager::IWavsServiceManagerInstance<
            (),
            SigningProvider,
        >;

    pub type IWavsServiceManagerQueryT =
        super::service_manager::IWavsServiceManager::IWavsServiceManagerInstance<(), QueryProvider>;
}

#[cfg(feature = "solidity-rpc")]
pub use rpc::*;
