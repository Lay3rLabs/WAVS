// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {ILayerServiceMulti} from "@layer-sdk/interfaces/ILayerServiceMulti.sol";
import {ILayerService} from "@layer-sdk/interfaces/ILayerService.sol";
import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";

/**
 * @title LayerServiceManager
 * @notice Concrete contract that:
 *  1) Validates signatures using the ECDSAStakeRegistry.
 *  2) Delegates the business logic (how payloads are actually handled) to a separate handler contract.
 */
contract LayerServiceManager is ECDSAServiceManagerBase,ILayerServiceMulti {
    // ------------------------------------------------------------------------
    // State
    // ------------------------------------------------------------------------
    /// @notice The external contract to which payload-handling logic is delegated.
    address public immutable serviceHandler;

    // ------------------------------------------------------------------------
    // Custom Errors
    // ------------------------------------------------------------------------
    error InvalidSignature();

    // ------------------------------------------------------------------------
    // Constructor
    // ------------------------------------------------------------------------
    constructor(
        address _avsDirectory,
        address _stakeRegistry,
        address _rewardsCoordinator,
        address _delegationManager,
        address _serviceHandler
    )
        ECDSAServiceManagerBase(
            _avsDirectory,
            _stakeRegistry,
            _rewardsCoordinator,
            _delegationManager
        )
    {
        require(_serviceHandler != address(0), "Invalid service handler address");
        serviceHandler = _serviceHandler;
    }

    // ------------------------------------------------------------------------
    // Functions
    // ------------------------------------------------------------------------

    /**
     * @notice Single-payload version of addPayload
     */
    function handleSignedData(bytes calldata data, bytes calldata signature) external
    {
        require(_validate(data, signature), "Invalid signature");
        _delegateHandleAddPayload(data, signature);
    }

    /**
     * @notice Multi-payload version of addPayload
     */
    function handleSignedDataMulti(bytes[] calldata datas, bytes[] calldata signatures) external
    {
        require(_validateMulti(datas, signatures), "Invalid signature");

        for (uint256 i = 0; i < datas.length; i++) {
            _delegateHandleAddPayload(datas[i], signatures[i]);
        }
    }

    /**
     * @notice Validate a single payload's signature via ECDSAStakeRegistry.
     */
    function _validate(bytes calldata data, bytes calldata signature) internal view returns (bool)
    {
        bytes32 message = keccak256(data);
        bytes32 ethSignedMessageHash = ECDSAUpgradeable.toEthSignedMessageHash(message);
        bytes4 magicValue = IERC1271Upgradeable.isValidSignature.selector;

        // If the registry returns the magicValue, signature is considered valid
        return (
            magicValue ==
            ECDSAStakeRegistry(stakeRegistry).isValidSignature(
                ethSignedMessageHash,
                signature
            )
        );
    }

    function _validateMulti(bytes[] calldata datas, bytes[] calldata signatures) internal view returns (bool)
    {
        if (datas.length != signatures.length) {
            return false;
        }
        for (uint256 i = 0; i < datas.length; i++) {
            if (!_validate(datas[i], signatures[i])) {
                return false;
            }
        }
        return true;
    }

    /**
     * @dev Internal function to delegate payload handling to the external handler contract.
     * @param data The signed data
     * @param signature The signature of `data`
     */
    function _delegateHandleAddPayload(bytes calldata data, bytes calldata signature)
        internal
    {
        // If you want to impose additional checks, you can do them here
        ILayerService(serviceHandler).handleSignedData(data, signature);
    }
}
