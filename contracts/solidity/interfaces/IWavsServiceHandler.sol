// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IWavsServiceHandler {
    struct SignatureData {
        address[] operators;
        bytes[] signatures;
        uint32 referenceBlock;
    }
    struct Envelope {
        bytes payload;
        uint256 eventId;
    }

    /**
     * @param envelope The envelope containing the data.
     * @param signatureData The signature data.
     */
    function handleSignedEnvelope(Envelope calldata envelope, SignatureData calldata signatureData) external;
}
