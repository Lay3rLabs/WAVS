// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {IWavsServiceHandler} from "../interfaces/bls/IWavsServiceHandler.sol";
import {IWavsServiceManager} from "../interfaces/bls/IWavsServiceManager.sol";
import {ISimpleTrigger} from "./ISimpleTrigger.sol";

/**
 * @title SimpleBlsSubmit
 * @author Lay3r Labs
 * @notice Contract for BLS-signed submission handling
 * @dev This contract implements the BLS variant of IWavsServiceHandler.
 *      It does NOT implement ISimpleSubmit because that interface references
 *      the ECDSA SignatureData type. BLS tests verify via isValidTriggerId()
 *      after the BLS pairing check passes in _SERVICE_MANAGER.validate().
 */
contract SimpleBlsSubmit is IWavsServiceHandler {
    IWavsServiceManager private immutable _SERVICE_MANAGER;

    /// @notice Mapping from trigger ID to valid triggers
    mapping(ISimpleTrigger.TriggerId => bool) public validTriggers;

    /// @notice DataWithId is a struct containing a trigger ID and data (mirrors ISimpleSubmit.DataWithId)
    struct DataWithId {
        ISimpleTrigger.TriggerId triggerId;
        bytes data;
    }

    /**
     * @notice Constructor
     * @param serviceManager The BLS service manager
     */
    constructor(IWavsServiceManager serviceManager) {
        _SERVICE_MANAGER = serviceManager;
    }

    /// @inheritdoc IWavsServiceHandler
    function handleSignedEnvelope(
        IWavsServiceHandler.Envelope calldata envelope,
        IWavsServiceHandler.SignatureData calldata signatureData
    ) external {
        _SERVICE_MANAGER.validate(envelope, signatureData);

        DataWithId memory dataWithId = abi.decode(envelope.payload, (DataWithId));
        validTriggers[dataWithId.triggerId] = true;
    }

    /**
     * @notice Checks if a trigger ID is valid (BLS signature was verified)
     * @param triggerId The trigger ID to check
     * @return True if the trigger ID has been verified, false otherwise
     */
    function isValidTriggerId(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (bool) {
        return validTriggers[triggerId];
    }

    /// @inheritdoc IWavsServiceHandler
    function getServiceManager() external view returns (address) {
        return address(_SERVICE_MANAGER);
    }
}
