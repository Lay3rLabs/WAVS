use alloy::{
    sol,
    transports::http::{Client, Http},
};
use hello_world::HelloWorldServiceManager::HelloWorldServiceManagerInstance;

use crate::eigen_client::solidity_types::HttpSigningProvider;

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

pub type HelloWorldServiceManagerT =
    HelloWorldServiceManagerInstance<Http<Client>, HttpSigningProvider>;
