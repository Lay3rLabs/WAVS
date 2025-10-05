use alloy_node_bindings::AnvilInstance;
use alloy_primitives::Address;
use alloy_provider::DynProvider;
use tempfile::TempDir;
use wavs_types::{ChainKey, Credential};

use crate::{
    evm_client::{EvmSigningClient, EvmSigningClientConfig},
    test_utils::anvil::safe_spawn_anvil,
};

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

/// Test dependencies for EVM contract testing
/// Provides a reusable setup for testing with simple service manager and handler contracts
pub struct TestContractDeps {
    pub _anvil: AnvilInstance,
    pub _data_dir: TempDir,
    pub client: EvmSigningClient,
    pub chain: ChainKey,
}

impl TestContractDeps {
    /// Create a new test environment with Anvil and EVM client
    pub async fn new() -> Self {
        let anvil = safe_spawn_anvil();
        let data_dir = tempfile::tempdir().unwrap();
        let chain = ChainKey::new("evm:local").unwrap();

        // Create EVM client directly
        let endpoint = anvil.endpoint().parse().unwrap();
        let client_config = EvmSigningClientConfig::new(
            endpoint,
            Credential::new(
                "test test test test test test test test test test test junk".to_string(),
            ),
        );

        let client = EvmSigningClient::new(client_config).await.unwrap();

        Self {
            _anvil: anvil,
            _data_dir: data_dir,
            client,
            chain,
        }
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
