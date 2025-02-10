// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface ILayerServiceManager {
    // ------------------------------------------------------------------------
    // Custom Errors
    // ------------------------------------------------------------------------
    error InvalidSignature();

    /**
     * @param data The arbitrary data that was signed.
     * @param signature The signature of the data.
     */
    function validate(bytes calldata data, bytes calldata signature) external view;
}