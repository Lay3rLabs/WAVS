// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {IWavsServiceHandler} from "../interfaces/IWavsServiceHandler.sol";
import {IWavsServiceManager} from "../interfaces/IWavsServiceManager.sol";
import {ISimpleTrigger} from "./ISimpleTrigger.sol";
import {ISimpleSubmit} from "./ISimpleSubmit.sol";

contract SimpleSubmit is IWavsServiceHandler, ISimpleSubmit {
    IWavsServiceManager private immutable _SERVICE_MANAGER;

    mapping(ISimpleTrigger.TriggerId => bool) public validTriggers;
    mapping(ISimpleTrigger.TriggerId => ISimpleSubmit.SignedData) public signedDatas;

    constructor(
        IWavsServiceManager serviceManager
    ) {
        _SERVICE_MANAGER = serviceManager;
    }

    function handleSignedEnvelope(
        IWavsServiceHandler.Envelope calldata envelope,
        IWavsServiceHandler.SignatureData calldata signatureData
    ) external {
        _SERVICE_MANAGER.validate(envelope, signatureData);

        ISimpleSubmit.DataWithId memory dataWithId =
            abi.decode(envelope.payload, (ISimpleSubmit.DataWithId));

        signedDatas[dataWithId.triggerId] = ISimpleSubmit.SignedData({
            data: dataWithId.data,
            signatureData: signatureData,
            envelope: envelope
        });

        validTriggers[dataWithId.triggerId] = true;
    }

    function isValidTriggerId(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (bool) {
        return validTriggers[triggerId];
    }

    function getSignedData(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (ISimpleSubmit.SignedData memory signedData) {
        signedData = signedDatas[triggerId];
    }

    // not really needed, just to make alloy generate DataWithId
    function getDataWithId(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (ISimpleSubmit.DataWithId memory dataWithId) {
        ISimpleSubmit.SignedData memory signedData = signedDatas[triggerId];
        dataWithId = ISimpleSubmit.DataWithId({triggerId: triggerId, data: signedData.data});
    }

    function getServiceManager() external view returns (address) {
        return address(_SERVICE_MANAGER);
    }
}
