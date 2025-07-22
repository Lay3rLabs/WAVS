// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

/**
 * @title IWavsOperatorUpdateHandler
 * @author Lay3r Labs
 * @notice Interface for the operator weight sync handler
 * @dev This interface defines the types for the operator weight sync handler
 */
interface IWavsOperatorUpdateHandler {
    /**
     * @notice OperatorUpdatePayload is a struct containing the operators to update
     * @param operatorsPerQuorum The operators per quorum
     * @param quorumNumbers The quorum numbers
     */
    struct OperatorUpdatePayload {
        address[][] operatorsPerQuorum;
        bytes quorumNumbers;
    }
}
