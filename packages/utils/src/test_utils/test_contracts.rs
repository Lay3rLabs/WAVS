use alloy_node_bindings::{Anvil, AnvilInstance};
use alloy_primitives::Address;
use alloy_provider::DynProvider;
use tempfile::TempDir;
use wavs_types::ChainName;

use crate::evm_client::EvmSigningClient;

pub mod service_manager {
    use alloy_sol_types::sol;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleServiceManager,
        "../../examples/contracts/solidity/abi/SimpleServiceManager.sol/SimpleServiceManager.json"
    );

    pub use SimpleServiceManager::*;
}

pub mod service_handler {
    use alloy_sol_types::sol;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleSubmit,
        "../../examples/contracts/solidity/abi/SimpleSubmit.sol/SimpleSubmit.json"
    );

    pub use SimpleSubmit::*;
}

pub use service_handler::{
    ISimpleSubmit, SimpleSubmit as SimpleServiceHandler,
    SimpleSubmitInstance as SimpleServiceHandlerInstance,
};
pub use service_manager::{SimpleServiceManager, SimpleServiceManagerInstance};

/// Test dependencies for EVM contract testing
/// Provides a reusable setup for testing with simple service manager and handler contracts
pub struct TestContractDeps {
    pub _anvil: AnvilInstance,
    pub _data_dir: TempDir,
    pub client: EvmSigningClient,
    pub chain_name: ChainName,
}

impl TestContractDeps {
    /// Create a new test environment with Anvil and EVM client
    pub async fn new() -> Self {
        let anvil = Anvil::new().spawn();
        let data_dir = tempfile::tempdir().unwrap();
        let chain_name = ChainName::new("local").unwrap();

        // Create EVM client directly
        let endpoint_url = anvil.endpoint().parse().unwrap();
        let client_config = crate::evm_client::EvmSigningClientConfig {
            endpoint: crate::evm_client::EvmEndpoint::Http(endpoint_url),
            credential: "test test test test test test test test test test test junk".to_string(),
            hd_index: None,
            gas_estimate_multiplier: None,
            poll_interval: None,
        };

        let client = EvmSigningClient::new(client_config).await.unwrap();

        Self {
            _anvil: anvil,
            _data_dir: data_dir,
            client,
            chain_name,
        }
    }

    /// Deploy a simple service manager contract for testing
    pub async fn deploy_simple_service_manager(&self) -> SimpleServiceManagerInstance<DynProvider> {
        SimpleServiceManager::deploy(self.client.provider.clone())
            .await
            .unwrap()
    }

    /// Deploy a simple service handler contract for testing
    pub async fn deploy_simple_service_handler(
        &self,
        service_manager_address: Address,
    ) -> SimpleServiceHandlerInstance<DynProvider> {
        let instance =
            SimpleServiceHandler::deploy(self.client.provider.clone(), service_manager_address)
                .await
                .unwrap();

        assert_eq!(
            instance.getServiceManager().call().await.unwrap(),
            service_manager_address,
            "Service manager address mismatch"
        );

        instance
    }
}
