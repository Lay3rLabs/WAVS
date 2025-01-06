// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import {ILayerTrigger} from "./ILayerTrigger.sol";

interface ILayerServiceManager {
    struct Payload {
        ILayerTrigger.TriggerId triggerId;
        bytes data;
    }

    struct SignedPayload {
        Payload payload;
        bytes signature;
    }
}
