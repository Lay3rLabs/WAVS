// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IWavsServiceHandler {
    struct SignatureData {
        address[] signers;
        bytes[] signatures;
        uint32 referenceBlock;
    }
    struct Envelope {
        bytes20 eventId;
        // currently unused, for future version. added now for padding
        bytes12 ordering;
        bytes payload;
    }

    /**
     * @param envelope The envelope containing the data.
     * @param signatureData The signature data.
     */
    function handleSignedEnvelope(Envelope calldata envelope, SignatureData calldata signatureData) external;
}
