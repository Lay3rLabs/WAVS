pragma solidity ^0.8.0;

import {LayerServiceManager} from "@layer-contracts/LayerServiceManager.sol";
import {ILayerServiceManager} from "@layer-contracts/interfaces/ILayerServiceManager.sol";
import {ISimpleTrigger} from "./interfaces/ISimpleTrigger.sol";
import {ISimpleSubmit} from "./interfaces/ISimpleSubmit.sol";

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
    mapping(ISimpleTrigger.TriggerId => bytes) datas;
    mapping(ISimpleTrigger.TriggerId => bytes) signatures;

    // payload data is expected to be DataWithId
    function _handleAddPayload(ILayerServiceManager.SignedPayload calldata signedPayload) internal virtual override { 
        ISimpleSubmit.DataWithId memory dataWithId = abi.decode(signedPayload.data, (ISimpleSubmit.DataWithId));

        signatures[dataWithId.triggerId] = signedPayload.signature;
        datas[dataWithId.triggerId] = dataWithId.data;
        validTriggers[dataWithId.triggerId] = true;
    }

    function isValidTriggerId(ISimpleTrigger.TriggerId triggerId) external view returns (bool) {
        return validTriggers[triggerId];
    }

    function getSignature(ISimpleTrigger.TriggerId triggerId) external view returns (bytes memory signature) {
        signature = signatures[triggerId];
    }

    function getData(ISimpleTrigger.TriggerId triggerId) external view returns (bytes memory data) {
        data = datas[triggerId];
    }
}