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

pub mod example_service_manager {
    use alloy_sol_types::sol;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleServiceManager,
        "../../examples/contracts/solidity/abi/SimpleServiceManager.sol/SimpleServiceManager.json"
    );
}

pub type SimpleTriggerT = SimpleTriggerInstance<DynProvider>;
pub type SimpleSubmitT = SimpleSubmitInstance<DynProvider>;
pub type SimpleServiceManagerT = SimpleSubmitInstance<DynProvider>;
