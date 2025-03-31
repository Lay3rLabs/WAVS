// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IWavsServiceManager} from "../../../contracts/solidity/interfaces/IWavsServiceManager.sol";
import {IWavsServiceHandler} from "../../../contracts/solidity/interfaces/IWavsServiceHandler.sol";

contract SimpleServiceManager is IWavsServiceManager {
    string private serviceURI;
    function validate(IWavsServiceHandler.Envelope calldata envelope, IWavsServiceHandler.SignatureData calldata signatureData) external view {
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
}