// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

interface IManagerUpdateTypes {
    error InvalidTriggerId(uint64 expectedTriggerId);

    /// @notice DataWithId is a struct containing a trigger ID and updated operator info
    struct UpdateWithId {
        uint64 triggerId;
        uint256 numerator;
        uint256 denominator;
    }
}
