// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IServiceHandler} from "@layer-sdk/interfaces/IServiceHandler.sol";
import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";
import {ILayerServiceManager} from "./interfaces/ILayerServiceManager.sol";

/**
 * @title LayerServiceManager
 * @notice Concrete contract that:
 *  1) Validates signatures using the ECDSAStakeRegistry.
 *  2) Delegates the business logic (how payloads are actually handled) to a separate handler contract.
 */
contract LayerServiceManager is ECDSAServiceManagerBase {
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
     * @param signedPayload Struct containing the data and signature
     */
    function addPayload(
        ILayerServiceManager.SignedPayload calldata signedPayload
    )
        external
    {
        require(validatePayload(signedPayload), "Invalid signature");
        _delegateHandleAddPayload(signedPayload.data, signedPayload.signature);
    }

    /**
     * @notice Multi-payload version of addPayload
     * @param signedPayloads Array of SignedPayload structs
     */
    function addPayloadMulti(
        ILayerServiceManager.SignedPayload[] calldata signedPayloads
    )
        external
    {
        require(validatePayloadMulti(signedPayloads), "Invalid signature");

        for (uint256 i = 0; i < signedPayloads.length; i++) {
            _delegateHandleAddPayload(signedPayloads[i].data, signedPayloads[i].signature);
        }
    }

    /**
     * @notice Validate a single payload's signature via ECDSAStakeRegistry.
     * @param signedPayload Struct containing the data and signature
     */
    function validatePayload(
        ILayerServiceManager.SignedPayload calldata signedPayload
    )
        public
        view
        returns (bool)
    {
        bytes32 message = keccak256(signedPayload.data);
        bytes32 ethSignedMessageHash = ECDSAUpgradeable.toEthSignedMessageHash(message);
        bytes4 magicValue = IERC1271Upgradeable.isValidSignature.selector;

        // If the registry returns the magicValue, signature is considered valid
        return (
            magicValue ==
            ECDSAStakeRegistry(stakeRegistry).isValidSignature(
                ethSignedMessageHash,
                signedPayload.signature
            )
        );
    }

    /**
     * @notice Validate multiple payloads' signatures via ECDSAStakeRegistry.
     * @param signedPayloads Array of SignedPayload structs containing the data and signature
     */
    function validatePayloadMulti(
        ILayerServiceManager.SignedPayload[] calldata signedPayloads
    )
        public
        view
        returns (bool)
    {
        for (uint256 i = 0; i < signedPayloads.length; i++) {
            if (!validatePayload(signedPayloads[i])) {
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
        IServiceHandler(serviceHandler).handleAddPayload(data, signature);
    }
}
