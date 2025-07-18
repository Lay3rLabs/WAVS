// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

interface IMirrorUpdateTypes {
    error InvalidTriggerId(uint64 expectedTriggerId);

    /// @notice DataWithId is a struct containing a trigger ID and updated operator info
    struct UpdateWithId {
        uint64 triggerId;
        uint256 thresholdWeight;
        address[] operators;
        address[] signingKeyAddresses;
        uint256[] weights;
    }
}
