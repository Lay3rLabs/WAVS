// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IWavsServiceManager} from "@layer/interfaces/IWavsServiceManager.sol";
import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";

/**
 * @title WavsServiceManager
 * @notice contract that validates signatures using the ECDSAStakeRegistry.
 * typically this contract is called from some "IWavsServiceHandler" (a vanilla implementation of ILayerService/ILayerServiceMulti)
 */
contract WavsServiceManager is ECDSAServiceManagerBase,IWavsServiceManager {

    // ------------------------------------------------------------------------
    // Constructor
    // ------------------------------------------------------------------------
    constructor(
        address _avsDirectory,
        address _stakeRegistry,
        address _rewardsCoordinator,
        address _delegationManager
    )
        ECDSAServiceManagerBase(
            _avsDirectory,
            _stakeRegistry,
            _rewardsCoordinator,
            _delegationManager
        )
    {
    }

    // ------------------------------------------------------------------------
    // Functions
    // ------------------------------------------------------------------------

    /**
     * @notice Validate signed data via ECDSAStakeRegistry.
     */
    function validate(bytes calldata data, bytes calldata signature) external view
    {
        bytes32 message = keccak256(data);
        bytes32 ethSignedMessageHash = ECDSAUpgradeable.toEthSignedMessageHash(message);
        bytes4 magicValue = IERC1271Upgradeable.isValidSignature.selector;

        // If the registry returns the magicValue, signature is considered valid
        if( magicValue !=
            ECDSAStakeRegistry(stakeRegistry).isValidSignature(
                ethSignedMessageHash,
                signature
            )
        ) {
            revert IWavsServiceManager.InvalidSignature();
        }
    }
}
