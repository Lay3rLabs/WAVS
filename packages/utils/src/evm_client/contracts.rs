use alloy_primitives::Address;
use wavs_types::{
    BlsServiceHandlerInstance, BlsServiceHandlerRpc, BlsServiceManagerInstance,
    BlsServiceManagerRpc, IWavsServiceHandler, IWavsServiceHandlerQueryT,
    IWavsServiceHandlerSigningT, IWavsServiceManager, IWavsServiceManagerQueryT,
    IWavsServiceManagerSigningT,
};

use super::{EvmQueryClient, EvmSigningClient};

impl EvmSigningClient {
    pub fn service_handler(&self, address: Address) -> IWavsServiceHandlerSigningT {
        IWavsServiceHandler::new(address, self.provider.clone())
    }

    pub fn service_manager(&self, address: Address) -> IWavsServiceManagerSigningT {
        IWavsServiceManager::new(address, self.provider.clone())
    }

    pub fn bls_service_handler(&self, address: Address) -> BlsServiceHandlerInstance {
        BlsServiceHandlerRpc::new(address, self.provider.clone())
    }

    pub fn bls_service_manager(&self, address: Address) -> BlsServiceManagerInstance {
        BlsServiceManagerRpc::new(address, self.provider.clone())
    }
}

impl EvmQueryClient {
    pub fn service_handler(&self, address: Address) -> IWavsServiceHandlerQueryT {
        IWavsServiceHandler::new(address, self.provider.clone())
    }

    pub fn service_manager(&self, address: Address) -> IWavsServiceManagerQueryT {
        IWavsServiceManager::new(address, self.provider.clone())
    }

    pub fn bls_service_handler(&self, address: Address) -> BlsServiceHandlerInstance {
        BlsServiceHandlerRpc::new(address, self.provider.clone())
    }

    pub fn bls_service_manager(&self, address: Address) -> BlsServiceManagerInstance {
        BlsServiceManagerRpc::new(address, self.provider.clone())
    }
}
