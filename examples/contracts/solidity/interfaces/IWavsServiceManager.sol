// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.27;

import {IWavsServiceHandler} from "./IWavsServiceHandler.sol";

/**
 * @title IWavsServiceManager
 * @author Lay3r Labs
 * @notice Interface for the Wavs service manager
 * @dev This interface defines the functions and events for the Wavs service manager
 */
interface IWavsServiceManager {
    // ------------------------------------------------------------------------
    // Custom Errors
    // ------------------------------------------------------------------------
    /// @notice The error for the invalid signature length.
    error InvalidSignatureLength();
    /// @notice The error for the invalid signature block.
    error InvalidSignatureBlock();
    /// @notice The error for the invalid signature order.
    error InvalidSignatureOrder();
    /// @notice The error for the invalid signature.
    error InvalidSignature();
    /// @notice The error for the insufficient quorum zero.
    error InsufficientQuorumZero();
    /**
     * @notice The error for the insufficient quorum
     * @param signerWeight The weight of the signer
     * @param thresholdWeight The threshold weight
     * @param totalWeight The total weight
     */
    error InsufficientQuorum(uint256 signerWeight, uint256 thresholdWeight, uint256 totalWeight);
    /// @notice The error for the invalid quorum parameters.
    error InvalidQuorumParameters();

    // ------------------------------------------------------------------------
    // Events
    // ------------------------------------------------------------------------
    /**
     * @notice Event emitted when the service URI is updated
     * @param serviceURI The service URI
     */
    event ServiceURIUpdated(string serviceURI);
    /**
     * @notice Event emitted when the quorum threshold is updated
     * @param numerator The numerator of the quorum threshold
     * @param denominator The denominator of the quorum threshold
     */
    event QuorumThresholdUpdated(uint256 indexed numerator, uint256 indexed denominator);

    // ------------------------------------------------------------------------
    // Stake Registry View Functions
    // ------------------------------------------------------------------------
    /**
     * @notice Gets the operator's current weight
     * @param operator The address of the operator
     * @return The current weight of the operator
     */
    function getOperatorWeight(
        address operator
    ) external view returns (uint256);

    /**
     * @notice Validates a signed envelope
     * @param envelope The envelope containing the data.
     * @param signatureData The signature data.
     */
    function validate(
        IWavsServiceHandler.Envelope calldata envelope,
        IWavsServiceHandler.SignatureData calldata signatureData
    ) external view;

    /**
     * @notice Returns the service URI
     * @return The service URI.
     */
    function getServiceURI() external view returns (string memory);

    /**
     * @notice Sets the service URI
     * @param _serviceURI The service URI to update.
     */
    function setServiceURI(
        string calldata _serviceURI
    ) external;

    /**
     * @notice Returns the latest operator address associated with a signing key.
     * @param signingKeyAddress The address of the signing key.
     * @return The latest operator address associated with the signing key, or address(0) if none.
     */
    function getLatestOperatorForSigningKey(
        address signingKeyAddress
    ) external view returns (address);

    /**
     * @notice Returns the allocation manager address.
     * @return The allocation manager address.
     */
    function getAllocationManager() external view returns (address);

    /**
     * @notice Returns the delegation manager address.
     * @return The delegation manager address.
     */
    function getDelegationManager() external view returns (address);

    /**
     * @notice Returns the stake registry address.
     * @return The stake registry address.
     */
    function getStakeRegistry() external view returns (address);
}
