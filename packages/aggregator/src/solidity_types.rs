use alloy::sol;
pub mod erc1271 {

    use super::*;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        IERC1271,
        "../../contracts/abi/IERC1271.sol/IERC1271.json"
    );
}
