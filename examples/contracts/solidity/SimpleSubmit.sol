pragma solidity ^0.8.0;

import {LayerServiceManager} from "@layer-contracts/LayerServiceManager.sol";
import {ILayerServiceManager} from "@layer-contracts/interfaces/ILayerServiceManager.sol";
import {ISimpleTrigger} from "./interfaces/ISimpleTrigger.sol";

contract SimpleSubmit is LayerServiceManager {
    constructor(
        address _avsDirectory,
        address _stakeRegistry,
        address _rewardsCoordinator,
        address _delegationManager
    )
        LayerServiceManager(
            _avsDirectory,
            _stakeRegistry,
            _rewardsCoordinator,
            _delegationManager
        )
    {}

    mapping(ISimpleTrigger.TriggerId => bool) validTriggers;
    mapping(ISimpleTrigger.TriggerId => ILayerServiceManager.SignedPayload) payloadsByTriggerId;

    function _handleAddPayload(ILayerServiceManager.SignedPayload calldata signedPayload) internal virtual override { 
        ISimpleTrigger.TriggerInfo memory triggerInfo = abi.decode(signedPayload.data, (ISimpleTrigger.TriggerInfo));
        validTriggers[triggerInfo.triggerId] = true;
    }

    function isValidTriggerId(ISimpleTrigger.TriggerId triggerId) external view returns (bool) {
        return validTriggers[triggerId];
    }

    function getSignedPayloadForTriggerId(ISimpleTrigger.TriggerId triggerId) external view returns (ILayerServiceManager.SignedPayload memory signedPayload) {
        signedPayload = payloadsByTriggerId[triggerId];
    }
}