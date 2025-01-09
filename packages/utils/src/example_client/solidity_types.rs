use alloy::transports::BoxTransport;
use example_submit::SimpleSubmit::SimpleSubmitInstance;
use example_trigger::SimpleTrigger::SimpleTriggerInstance;

use crate::eigen_client::solidity_types::BoxSigningProvider;

pub mod example_trigger {
    use alloy::sol;
    pub use ISimpleTrigger::TriggerInfo;
    pub use SimpleTrigger::NewTriggerId;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleTrigger,
        "../../examples/contracts/solidity/abi/SimpleTrigger.sol/SimpleTrigger.json"
    );
}

pub mod example_submit {
    use alloy::sol;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleSubmit,
        "../../examples/contracts/solidity/abi/SimpleSubmit.sol/SimpleSubmit.json"
    );
}

pub type SimpleTriggerT = SimpleTriggerInstance<BoxTransport, BoxSigningProvider>;
pub type SimpleSubmitT = SimpleSubmitInstance<BoxTransport, BoxSigningProvider>;
