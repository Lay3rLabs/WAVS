// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {IWavsServiceHandler} from "../interfaces/IWavsServiceHandler.sol";
import {IWavsServiceManager} from "../interfaces/IWavsServiceManager.sol";

/**
 * @title TestServiceManager
 * @notice Minimal IWavsServiceManager implementation for integration testing.
 *         Performs no validation and exposes helper setters for configuring weights.
 */
contract TestServiceManager is IWavsServiceManager {
    string private _serviceURI;
    mapping(address => uint256) private _operatorWeights;

    function getOperatorWeight(address operator) external view override returns (uint256) {
        return _operatorWeights[operator];
    }

    function validate(
        IWavsServiceHandler.Envelope calldata,
        IWavsServiceHandler.SignatureData calldata
    ) external pure override {
        // Intentionally no-op: tests can rely on successful validation by default.
    }

    function getServiceURI() external view override returns (string memory) {
        return _serviceURI;
    }

    function setServiceURI(string calldata serviceURI) external override {
        _serviceURI = serviceURI;
        emit ServiceURIUpdated(serviceURI);
    }

    function setOperatorWeight(address operator, uint256 weight) external {
        _operatorWeights[operator] = weight;
    }

    function getLatestOperatorForSigningKey(address signingKeyAddress)
        external
        pure
        override
        returns (address)
    {
        return signingKeyAddress;
    }

    function getAllocationManager() external pure override returns (address) {
        return address(0);
    }

    function getDelegationManager() external pure override returns (address) {
        return address(0);
    }

    function getStakeRegistry() external pure override returns (address) {
        return address(0);
    }
}

/**
 * @title TestServiceHandler
 * @notice Minimal IWavsServiceHandler that defers validation to the configured manager.
 */
contract TestServiceHandler is IWavsServiceHandler {
    IWavsServiceManager private immutable _serviceManager;

    event EnvelopeHandled(bytes payload, address[] signers);

    constructor(IWavsServiceManager serviceManager) {
        _serviceManager = serviceManager;
    }

    function handleSignedEnvelope(
        Envelope calldata envelope,
        SignatureData calldata signatureData
    ) external override {
        _serviceManager.validate(envelope, signatureData);
        emit EnvelopeHandled(envelope.payload, signatureData.signers);
    }

    function getServiceManager() external view override returns (address) {
        return address(_serviceManager);
    }
}
