// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import {ISimpleTrigger} from "./ISimpleTrigger.sol";

interface ISimpleSubmit {
    struct DataWithId {
        ISimpleTrigger.TriggerId triggerId;
        bytes data;
    }

    // just to make alloy see the DataWithId struct
    function getDataWithId(ISimpleTrigger.TriggerId triggerId) external view returns (DataWithId memory);
}