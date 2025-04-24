// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IWavsServiceManager} from "../interfaces/IWavsServiceManager.sol";
import {IWavsServiceHandler} from "../interfaces/IWavsServiceHandler.sol";

contract SimpleServiceManager is IWavsServiceManager {
    string private serviceURI;

    function validate(
        IWavsServiceHandler.Envelope calldata envelope,
        IWavsServiceHandler.SignatureData calldata signatureData
    ) external view {
        // always valid, for demo purposes
    }

    function getServiceURI() external view returns (string memory) {
        return serviceURI;
    }

    /**
     * @param _serviceURI The service URI to update.
     */
    function setServiceURI(string calldata _serviceURI) external {
        serviceURI = _serviceURI;
        emit ServiceURIUpdated(_serviceURI);
    }

    function getOperatorWeight(
        address /* operator */
    ) external pure override returns (uint256) {
        return 1; // hard-coded at 1 for demo purposes
    }

    function getLastCheckpointThresholdWeight()
        external
        pure
        override
        returns (uint256)
    {
        return 3; // hard-coded at 3 for demo purposes
    }

    function getLastCheckpointTotalWeight() external pure override returns (uint256) {
        return 5; // hard-coded at 5 for demo purposes
    }
}
