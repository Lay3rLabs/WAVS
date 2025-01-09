use alloy_sol_types::SolValue;
use example_submit::DataWithId;
use example_trigger::TriggerInfo;

pub fn decode_trigger_input(input: Vec<u8>) -> std::result::Result<(u64, Vec<u8>), String> {
    TriggerInfo::abi_decode(&input, false)
        .map_err(|e| e.to_string())
        .map(|t| (t.triggerId, t.data.to_vec()))
}

pub fn encode_trigger_output(trigger_id: u64, output: impl AsRef<[u8]>) -> Vec<u8> {
    DataWithId {
        triggerId: trigger_id,
        data: output.as_ref().to_vec().into(),
    }
    .abi_encode()
}

mod example_trigger {
    use alloy_sol_macro::sol;
    pub use ISimpleTrigger::TriggerInfo;

    sol!(
        #[allow(missing_docs)]
        SimpleTrigger,
        "../../contracts/solidity/abi/SimpleTrigger.sol/SimpleTrigger.json"
    );
}

mod example_submit {
    use alloy_sol_macro::sol;
    pub use ISimpleSubmit::DataWithId;

    sol!(
        #[allow(missing_docs)]
        ISimpleSubmit,
        "../../contracts/solidity/abi/ISimpleSubmit.sol/ISimpleSubmit.json"
    );
}
