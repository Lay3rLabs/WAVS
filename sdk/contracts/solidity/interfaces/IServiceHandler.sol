// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IServiceHandler {
    /**
     * @notice Called by LayerServiceManager after successful payload signature validation.
     * @param data The arbitrary data that was signed.
     * @param signature The signature of the data.
     */
    function handleAddPayload(bytes calldata data, bytes calldata signature) external;
}
