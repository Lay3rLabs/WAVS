pub mod example_submit {
    use alloy_sol_macro::sol;
    pub use ISimpleSubmit::{DataWithId, SignedData};

    sol!(
        #[allow(missing_docs)]
        SimpleSubmit,
        "../../../contracts/solidity/abi/SimpleSubmit.sol/SimpleSubmit.json"
    );
}
