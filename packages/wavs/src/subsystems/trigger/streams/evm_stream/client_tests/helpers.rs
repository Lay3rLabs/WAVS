use alloy_node_bindings::AnvilInstance;
use alloy_provider::{DynProvider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::sol;

use crate::subsystems::trigger::evm_stream::client::EvmTriggerStreamsController;

pub async fn wait_for_all_rpc_requests_landed(controller: &EvmTriggerStreamsController) {
    while controller.subscriptions.any_active_rpcs_in_flight() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

pub struct EventEmitterClient {
    provider: DynProvider,
}

impl EventEmitterClient {
    pub fn new(anvil: &AnvilInstance, index: usize) -> Self {
        let wallet = PrivateKeySigner::from_signing_key(anvil.keys()[index].clone().into());

        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(anvil.endpoint().parse().unwrap());

        let provider = DynProvider::new(provider);

        Self { provider }
    }

    pub async fn deploy(&self) -> EventEmitter::EventEmitterInstance<DynProvider> {
        EventEmitter::deploy(self.provider.clone()).await.unwrap()
    }
}

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EventEmitter,
    "./tests/contracts/solidity/abi/EventEmitter.sol/EventEmitter.json"
);
