// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.9;

import "./IWavsServiceHandler.sol";

interface IWavsServiceManager {

    // ------------------------------------------------------------------------
    // Custom Errors
    // ------------------------------------------------------------------------
    error InvalidSignature();
    event ServiceURIUpdated(string serviceURI);

    // ------------------------------------------------------------------------
    // Stake Registry View Functions
    // ------------------------------------------------------------------------
    /**
     * @notice Gets the operator's current weight
     * @param operator The address of the operator
     * @return The current weight of the operator
     */
    function getOperatorWeight(address operator) external view returns (uint256);

    /**
     * @notice Gets the total weight from the last checkpoint
     * @return The total weight from the last checkpoint
     */
    function getLastCheckpointTotalWeight() external view returns (uint256);

    /**
     * @notice Gets the threshold weight from the last checkpoint
     * @return The threshold weight from the last checkpoint
     */
    function getLastCheckpointThresholdWeight() external view returns (uint256);
    /**
     * @param envelope The envelope containing the data.
     * @param signatureData The signature data.
     */
    function validate(IWavsServiceHandler.Envelope calldata envelope, IWavsServiceHandler.SignatureData calldata signatureData) external view;

    /**
     * @return The service URI.
     */
    function getServiceURI() external view returns (string memory);

    /**
     * @param _serviceURI The service URI to update.
     */
    function setServiceURI(string calldata _serviceURI) external;
}