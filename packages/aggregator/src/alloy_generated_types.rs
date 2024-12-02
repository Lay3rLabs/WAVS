//! Generated types and methods of the solidity contracts we use
//!
//! TODO: look for a better way to generate it, copy-pasting definitions from `.sol` might be a nightmare to maintain

use alloy::sol;

sol! {
    #[sol(rpc)]
    contract HelloWorldServiceManager {
        constructor(address) {}

        #[derive(Debug)]
        struct Task {
            string name;
            uint32 taskCreatedBlock;
        }

        #[derive(Debug)]
        function respondToTask(
            Task calldata task,
            uint32 referenceTaskIndex,
            bytes memory signature
        ) external;
    }
}
