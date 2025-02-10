// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {ILayerServiceHandler} from "@layer/interfaces/ILayerServiceHandler.sol";
import {ILayerServiceManager} from "@layer/interfaces/ILayerServiceManager.sol";
import {ISimpleTrigger} from "./interfaces/ISimpleTrigger.sol";
import {ISimpleSubmit} from "./interfaces/ISimpleSubmit.sol";

contract SimpleSubmit is ILayerServiceHandler {
    ILayerServiceManager private _serviceManager;

    mapping(ISimpleTrigger.TriggerId => bool) validTriggers;
    mapping(ISimpleTrigger.TriggerId => bytes) datas;
    mapping(ISimpleTrigger.TriggerId => bytes) signatures;

    constructor(ILayerServiceManager serviceManager) {
        _serviceManager = serviceManager;
    }

    function handleSignedData(bytes calldata data, bytes calldata signature) external {
        _serviceManager.validate(data, signature);

        ISimpleSubmit.DataWithId memory dataWithId = abi.decode(data, (ISimpleSubmit.DataWithId));

        signatures[dataWithId.triggerId] = signature;
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