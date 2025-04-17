// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IWavsServiceHandler} from "../../../contracts/solidity/interfaces/IWavsServiceHandler.sol";
import {IWavsServiceManager} from "../../../contracts/solidity/interfaces/IWavsServiceManager.sol";
import {ISimpleTrigger} from "./interfaces/ISimpleTrigger.sol";
import {ISimpleSubmit} from "./interfaces/ISimpleSubmit.sol";

contract SimpleSubmit is IWavsServiceHandler, ISimpleSubmit {
    IWavsServiceManager private _serviceManager;

    mapping(ISimpleTrigger.TriggerId => bool) validTriggers;
    mapping(ISimpleTrigger.TriggerId => ISimpleSubmit.SignedData) signedDatas;

    constructor(IWavsServiceManager serviceManager) {
        _serviceManager = serviceManager;
    }

    function handleSignedEnvelope(IWavsServiceHandler.Envelope calldata envelope, IWavsServiceHandler.SignatureData calldata signatureData) external {
        _serviceManager.validate(envelope, signatureData);

        ISimpleSubmit.DataWithId memory dataWithId = abi.decode(envelope.payload, (ISimpleSubmit.DataWithId));

        signedDatas[dataWithId.triggerId] = ISimpleSubmit.SignedData({
            data: dataWithId.data,
            signatureData: signatureData,
            envelope: envelope
        });

        validTriggers[dataWithId.triggerId] = true;
    }

    function isValidTriggerId(ISimpleTrigger.TriggerId triggerId) external view returns (bool) {
        return validTriggers[triggerId];
    }

    function getSignedData(ISimpleTrigger.TriggerId triggerId) external view returns (ISimpleSubmit.SignedData memory signedData) {
        signedData = signedDatas[triggerId];
    }
}
