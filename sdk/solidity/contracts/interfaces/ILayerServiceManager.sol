// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

interface ILayerServiceManager {
    struct SignedPayload {
        bytes data;
        bytes signature;
    }
}
