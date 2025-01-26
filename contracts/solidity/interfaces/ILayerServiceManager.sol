// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface ILayerServiceManager {
    struct SignedPayload {
        bytes data;
        bytes signature;
    }
}