// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

/**
 * @title IWavsServiceHandler
 * @author Lay3r Labs
 * @notice Interface for the Wavs service handler
 * @dev This interface defines the functions and events for the Wavs service handler
 */
interface IWavsServiceHandler {
    /// @notice The signature data struct
    struct SignatureData {
        address[] signers;
        bytes[] signatures;
        uint32 referenceBlock;
    }

    /// @notice The envelope struct
    struct Envelope {
        bytes20 eventId;
        // currently unused, for future version. added now for padding
        bytes12 ordering;
        bytes payload;
    }

    /**
     * @notice Handles a signed envelope
     * @param envelope The envelope containing the data.
     * @param signatureData The signature data.
     */
    function handleSignedEnvelope(
        Envelope calldata envelope,
        SignatureData calldata signatureData
    ) external;

    /**
     * @notice Returns the address of the service manager
     * @return The address of the service manager
     */
    function getServiceManager() external view returns (address);
}
