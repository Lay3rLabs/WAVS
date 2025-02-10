use example_submit::SimpleSubmit::SimpleSubmitInstance;
use example_trigger::SimpleTrigger::SimpleTriggerInstance;
use utils::eth_client::SigningProvider;

pub mod example_trigger {
    use alloy::sol;
    pub use SimpleTrigger::NewTrigger;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleTrigger,
        "../../examples/contracts/solidity/abi/SimpleTrigger.sol/SimpleTrigger.json"
    );
}

pub mod example_submit {
    use alloy::sol;
    pub use interface::ISimpleSubmit::DataWithId;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleSubmit,
        "../../examples/contracts/solidity/abi/SimpleSubmit.sol/SimpleSubmit.json"
    );

    mod interface {
        use alloy::sol;

        sol!(
            #[allow(missing_docs)]
            #[sol(rpc)]
            ISimpleSubmit,
            "../../examples/contracts/solidity/abi/ISimpleSubmit.sol/ISimpleSubmit.json"
        );
    }
}

pub type SimpleTriggerT = SimpleTriggerInstance<(), SigningProvider>;
pub type SimpleSubmitT = SimpleSubmitInstance<(), SigningProvider>;
