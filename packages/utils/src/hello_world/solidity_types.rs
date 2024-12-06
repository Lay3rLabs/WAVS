use alloy::{
    network::{Ethereum, EthereumWallet},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, RootProvider,
    },
    sol,
    transports::http::{Client, Http},
};
use hello_world::HelloWorldServiceManager::HelloWorldServiceManagerInstance;

pub mod stake_registry {
    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ECDSAStakeRegistry,
        "../../contracts/abi/ECDSAStakeRegistry.sol/ECDSAStakeRegistry.json"
    );
}

pub mod hello_world {
    use super::*;

    pub use IHelloWorldServiceManager::Task;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc, abi)]
        HelloWorldServiceManager,
        "../../contracts/abi/HelloWorldServiceManager.sol/HelloWorldServiceManager.json"
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

pub type HelloWorldServiceManagerT = HelloWorldServiceManagerInstance<
    Http<Client>,
    FillProvider<
        JoinFill<
            JoinFill<
                Identity,
                JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
            >,
            WalletFiller<EthereumWallet>,
        >,
        RootProvider<Http<Client>>,
        Http<Client>,
        Ethereum,
    >,
>;
