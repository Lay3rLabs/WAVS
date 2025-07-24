// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {IWavsServiceManager} from "../interfaces/IWavsServiceManager.sol";
import {IWavsServiceHandler} from "../interfaces/IWavsServiceHandler.sol";

/**
 * @title SimpleServiceManager
 * @author Lay3r Labs
 * @notice Contract for the simple service manager
 * @dev This contract implements the IWavsServiceManager interface
 */
contract SimpleServiceManager is IWavsServiceManager {
    string private serviceURI;

    mapping(address => uint256) private operatorWeights;
    uint256 private lastCheckpointThresholdWeight;
    uint256 private lastCheckpointTotalWeight;

    /// @inheritdoc IWavsServiceManager
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
        if (!(signatureData.referenceBlock < block.number)) {
            revert IWavsServiceManager.InvalidSignatureBlock();
        }
        if (!_validateOperatorSorting(signatureData.signers)) {
            revert IWavsServiceManager.InvalidSignatureOrder();
        }

        // Get the total operator weight of these signatures
        uint256 signedWeight = 0;
        for (uint256 i = 0; i < signatureData.signers.length; ++i) {
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
     * @notice Validates that operators are sorted in ascending byte order
     * @param operators Array of operator addresses
     * @return isValid True if the operators are properly sorted
     */
    function _validateOperatorSorting(
        address[] calldata operators
    ) internal pure returns (bool) {
        // Empty array or single element is always sorted
        if (!(operators.length > 1)) {
            return true;
        }

        // Check that each address is greater than the previous one
        for (uint256 i = 1; i < operators.length; ++i) {
            if (!(operators[i] > operators[i - 1])) {
                return false;
            }
        }

        return true;
    }

    /// @inheritdoc IWavsServiceManager
    function getServiceURI() external view returns (string memory) {
        return serviceURI;
    }

    /// @inheritdoc IWavsServiceManager
    function setServiceURI(
        string calldata _serviceURI
    ) external {
        serviceURI = _serviceURI;
        emit ServiceURIUpdated(_serviceURI);
    }

    /**
     * @notice Sets the weight of an operator
     * @param operator The operator
     * @param weight The weight of the operator
     */
    function setOperatorWeight(address operator, uint256 weight) external {
        operatorWeights[operator] = weight;
    }

    /**
     * @notice Sets the threshold weight of the last checkpoint
     * @param weight The threshold weight of the last checkpoint
     */
    function setLastCheckpointThresholdWeight(
        uint256 weight
    ) external {
        lastCheckpointThresholdWeight = weight;
    }

    /**
     * @notice Sets the total weight of the last checkpoint
     * @param weight The total weight of the last checkpoint
     */
    function setLastCheckpointTotalWeight(
        uint256 weight
    ) external {
        lastCheckpointTotalWeight = weight;
    }

    /// @inheritdoc IWavsServiceManager
    function getOperatorWeight(
        address operator
    ) external view returns (uint256) {
        return operatorWeights[operator];
    }

    /**
     * @notice Returns the threshold weight of the last checkpoint
     * @return The threshold weight of the last checkpoint
     */
    function getLastCheckpointThresholdWeight() external view returns (uint256) {
        return lastCheckpointThresholdWeight;
    }

    /**
     * @notice Returns the total weight of the last checkpoint
     * @return The total weight of the last checkpoint
     */
    function getLastCheckpointTotalWeight() external view returns (uint256) {
        return lastCheckpointTotalWeight;
    }

    /// @inheritdoc IWavsServiceManager
    function getLatestOperatorForSigningKey(
        address signingKeyAddress
    ) external pure override returns (address) {
        return signingKeyAddress;
    }

    /// @inheritdoc IWavsServiceManager
    function getDelegationManager() external pure returns (address) {
        return address(0);
    }

    /// @inheritdoc IWavsServiceManager
    function getAllocationManager() external pure returns (address) {
        return address(0);
    }

    /// @inheritdoc IWavsServiceManager
    function getStakeRegistry() external pure returns (address) {
        return address(0);
    }
}
