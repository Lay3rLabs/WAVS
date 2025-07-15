// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {IWavsServiceManager} from "../interfaces/IWavsServiceManager.sol";
import {IWavsServiceHandler} from "../interfaces/IWavsServiceHandler.sol";

contract SimpleServiceManager is IWavsServiceManager {
    string private serviceURI;

    mapping(address => uint256) private operatorWeights;
    uint256 private lastCheckpointThresholdWeight;
    uint256 private lastCheckpointTotalWeight;

    function validate(
        IWavsServiceHandler.Envelope calldata, /* envelope */
        IWavsServiceHandler.SignatureData calldata signatureData
    ) external view override {
        // Input validation
        if (
            signatureData.signers.length == 0
                || signatureData.signers.length != signatureData.signatures.length
        ) {
            revert IWavsServiceManager.InvalidSignatureLength();
        }
        if (signatureData.referenceBlock >= block.number) {
            revert IWavsServiceManager.InvalidSignatureBlock();
        }
        if (!_validateOperatorSorting(signatureData.signers)) {
            revert IWavsServiceManager.InvalidSignatureOrder();
        }

        // Get the total operator weight of these signatures
        uint256 signedWeight = 0;
        for (uint256 i = 0; i < signatureData.signers.length; i++) {
            signedWeight += operatorWeights[signatureData.signers[i]];
        }

        // Avoid 0 weight ever passing this check
        if (signedWeight == 0) {
            revert IWavsServiceManager.InsufficientQuorumZero();
        }

        // Check if the total weight meets the last checkpoint threshold
        if (signedWeight < lastCheckpointThresholdWeight) {
            revert IWavsServiceManager.InsufficientQuorum(
                signedWeight, lastCheckpointThresholdWeight, lastCheckpointTotalWeight
            );
        }
    }

    /**
     * @dev Validates that operators are sorted in ascending byte order
     * @param operators Array of operator addresses
     * @return True if the operators are properly sorted
     */
    function _validateOperatorSorting(
        address[] calldata operators
    ) internal pure returns (bool) {
        // Empty array or single element is always sorted
        if (operators.length <= 1) {
            return true;
        }

        // Check that each address is greater than the previous one
        for (uint256 i = 1; i < operators.length; i++) {
            if (operators[i] <= operators[i - 1]) {
                return false;
            }
        }

        return true;
    }

    function getServiceURI() external view returns (string memory) {
        return serviceURI;
    }

    function setServiceURI(
        string calldata _serviceURI
    ) external {
        serviceURI = _serviceURI;
        emit ServiceURIUpdated(_serviceURI);
    }

    function setOperatorWeight(address operator, uint256 weight) external {
        operatorWeights[operator] = weight;
    }

    function setLastCheckpointThresholdWeight(
        uint256 weight
    ) external {
        lastCheckpointThresholdWeight = weight;
    }

    function setLastCheckpointTotalWeight(
        uint256 weight
    ) external {
        lastCheckpointTotalWeight = weight;
    }

    function getOperatorWeight(
        address operator
    ) external view returns (uint256) {
        return operatorWeights[operator];
    }

    function getLastCheckpointThresholdWeight() external view returns (uint256) {
        return lastCheckpointThresholdWeight;
    }

    function getLastCheckpointTotalWeight() external view returns (uint256) {
        return lastCheckpointTotalWeight;
    }

    function getLatestOperatorForSigningKey(
        address signingKeyAddress
    ) external pure override returns (address) {
        return signingKeyAddress;
    }
}
