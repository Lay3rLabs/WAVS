use alloy_provider::DynProvider;
use example_submit::SimpleSubmit::SimpleSubmitInstance;
use example_trigger::SimpleTrigger::SimpleTriggerInstance;

pub mod example_trigger {
    use alloy_sol_types::sol;
    pub use SimpleTrigger::NewTrigger;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleTrigger,
        "../../examples/contracts/solidity/abi/SimpleTrigger.sol/SimpleTrigger.json"
    );
}

pub mod example_log_spam {
    use alloy_sol_types::sol;
    pub use LogSpam::Spam;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        LogSpam,
        "../../examples/contracts/solidity/abi/LogSpam.sol/LogSpam.json"
    );
}

pub mod example_submit {
    use alloy_sol_types::sol;
    pub use interface::ISimpleSubmit::SignedData;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleSubmit,
        "../../examples/contracts/solidity/abi/SimpleSubmit.sol/SimpleSubmit.json"
    );

    mod interface {
        use alloy_sol_types::sol;

        sol!(
            #[allow(missing_docs)]
            #[sol(rpc)]
            ISimpleSubmit,
            "../../examples/contracts/solidity/abi/ISimpleSubmit.sol/ISimpleSubmit.json"
        );
    }
}

pub type SimpleTriggerT = SimpleTriggerInstance<DynProvider>;
pub type SimpleSubmitT = SimpleSubmitInstance<DynProvider>;
pub use example_log_spam::LogSpam;
pub type LogSpamT = LogSpam::LogSpamInstance<DynProvider>;

// LogSpam client implementation
pub struct LogSpamClient {
    pub evm_client: utils::evm_client::EvmSigningClient,
    pub contract_address: alloy_primitives::Address,
    pub contract: LogSpamT,
}

impl LogSpamClient {
    pub fn new(
        evm_client: utils::evm_client::EvmSigningClient,
        contract_address: alloy_primitives::Address,
    ) -> Self {
        let contract = LogSpam::new(contract_address, evm_client.provider.clone());

        Self {
            evm_client,
            contract_address,
            contract,
        }
    }

    pub async fn new_deploy(
        evm_client: utils::evm_client::EvmSigningClient,
    ) -> anyhow::Result<Self> {
        let contract_address = Self::deploy(evm_client.provider.clone()).await?;
        Ok(Self::new(evm_client, contract_address))
    }

    pub async fn deploy(
        provider: alloy_provider::DynProvider,
    ) -> anyhow::Result<alloy_primitives::Address> {
        Ok(*LogSpam::deploy(provider).await?.address())
    }

    pub async fn emit_spam(
        &self,
        start_id: u64,
        count: u64,
    ) -> anyhow::Result<alloy_primitives::TxHash> {
        let pending = self
            .contract
            .emitSpam(
                alloy_primitives::U256::from(start_id),
                alloy_primitives::U256::from(count),
            )
            .send()
            .await?;
        Ok(*pending.tx_hash())
    }

    pub fn address(&self) -> alloy_primitives::Address {
        self.contract_address
    }

    pub fn evm_client(&self) -> &utils::evm_client::EvmSigningClient {
        &self.evm_client
    }
}
