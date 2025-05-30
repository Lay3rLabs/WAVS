// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.9;

import "./IWavsServiceHandler.sol";

interface IWavsServiceManager {

    // ------------------------------------------------------------------------
    // Custom Errors
    // ------------------------------------------------------------------------
    error InvalidSignature();
    error InsufficientQuorum();
    error InvalidQuorumParameters();
    
    // ------------------------------------------------------------------------
    // Events
    // ------------------------------------------------------------------------
    event ServiceURIUpdated(string serviceURI);
    event QuorumThresholdUpdated(uint256 numerator, uint256 denominator);

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

     /**
     * @notice Retrieves the latest operator address associated with a signing key.
     * @param signingKey The address of the signing key.
     * @return The latest operator address associated with the signing key, or address(0) if none.
     */
    function getLatestOperatorForSigningKey(address signingKey) external view returns(address);
}