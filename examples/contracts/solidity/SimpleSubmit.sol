// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IWavsServiceHandler} from "../../../contracts/solidity/interfaces/IWavsServiceHandler.sol";
import {IWavsServiceManager} from "../../../contracts/solidity/interfaces/IWavsServiceManager.sol";
import {ISimpleTrigger} from "./interfaces/ISimpleTrigger.sol";
import {ISimpleSubmit} from "./interfaces/ISimpleSubmit.sol";

contract SimpleSubmit is IWavsServiceHandler {
    IWavsServiceManager private _serviceManager;

    mapping(ISimpleTrigger.TriggerId => bool) validTriggers;
    mapping(ISimpleTrigger.TriggerId => bytes) datas;
    mapping(ISimpleTrigger.TriggerId => IWavsServiceHandler.SignatureData) signatures;

    constructor(IWavsServiceManager serviceManager) {
        _serviceManager = serviceManager;
    }

    function handleSignedEnvelope(IWavsServiceHandler.Envelope calldata envelope, IWavsServiceHandler.SignatureData calldata signatureData) external {
        _serviceManager.validate(envelope, signatureData);

        ISimpleSubmit.DataWithId memory dataWithId = abi.decode(envelope.payload, (ISimpleSubmit.DataWithId));

        signatures[dataWithId.triggerId] = signatureData;
        datas[dataWithId.triggerId] = dataWithId.data;
        validTriggers[dataWithId.triggerId] = true;
    }

    function isValidTriggerId(ISimpleTrigger.TriggerId triggerId) external view returns (bool) {
        return validTriggers[triggerId];
    }

    function getSignature(ISimpleTrigger.TriggerId triggerId) external view returns (IWavsServiceHandler.SignatureData memory signatureData) {
        signatureData = signatures[triggerId];
    }

    function getData(ISimpleTrigger.TriggerId triggerId) external view returns (bytes memory data) {
        data = datas[triggerId];
    }
}
