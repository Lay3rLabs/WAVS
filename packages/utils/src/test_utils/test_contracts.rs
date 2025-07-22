use alloy_network::TransactionBuilder;
use alloy_node_bindings::{Anvil, AnvilInstance};
use alloy_primitives::{utils::parse_ether, Address};
use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types_eth::TransactionRequest;
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::LocalSigner;
use tempfile::TempDir;
use wavs_types::ChainName;

use crate::{evm_client::EvmSigningClient, test_utils::{deploy_service_manager::{HexEncodedPrivateKey, ServiceManager, ServiceManagerConfig}, test_packet::mock_signer}};

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

    pub async fn create_signers<const N: usize>(&self) -> Vec<LocalSigner<SigningKey>> {
        let mut signers = Vec::with_capacity(N);

        for _ in 0..N {
            let signer = mock_signer();
            self.transfer_funds("100", signer.address()).await;
            signers.push(signer);
        }
        signers
    }

    pub async fn transfer_funds(&self, eth: &str, to: Address) {
        let amount = parse_ether(eth).unwrap();
        let tx = TransactionRequest::default()
            .with_from(self.client.signer.address())
            .with_to(to)
            .with_value(amount);

        self.client 
            .provider
            .send_transaction(tx)
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();
    }

    pub async fn deploy_service_manager(
        &self,
        config: ServiceManagerConfig,
    ) -> ServiceManager {
        let deployer_key = HexEncodedPrivateKey::new_random();

        self.transfer_funds("100", deployer_key.address)
            .await;

        ServiceManager::deploy(config, self._anvil.endpoint(), deployer_key)
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
