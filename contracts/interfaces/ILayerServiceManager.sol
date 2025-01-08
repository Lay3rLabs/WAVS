// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import {ILayerTrigger} from "./ILayerTrigger.sol";

interface ILayerServiceManager {
    struct SignedPayload {
        bytes data;
        bytes signature;
    }
}
