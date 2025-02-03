// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface ILayerService {
    /**
     * @param data The arbitrary data that was signed.
     * @param signature The signature of the data.
     */
    function handleSignedData(bytes calldata data, bytes calldata signature) external;
}
