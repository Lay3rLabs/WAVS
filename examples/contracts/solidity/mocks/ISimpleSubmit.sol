// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {ISimpleTrigger} from "./ISimpleTrigger.sol";
import {IWavsServiceHandler} from "../interfaces/IWavsServiceHandler.sol";

interface ISimpleSubmit {
    struct DataWithId {
        ISimpleTrigger.TriggerId triggerId;
        bytes data;
    }

    struct SignedData {
        bytes data;
        IWavsServiceHandler.SignatureData signatureData;
        IWavsServiceHandler.Envelope envelope;
    }

    function getSignedData(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (SignedData memory);

    // just so alloy can see the generated type
    function getDataWithId(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (DataWithId memory);
}
