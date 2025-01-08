// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

interface ILayerTrigger {
    struct TriggerResponse {
        TriggerId triggerId;
        address creator;
        bytes data;
    }

    type TriggerId is uint64;

    function getTrigger(
        TriggerId triggerId
    ) external view returns (TriggerResponse memory);
    function safeMint(address to, string memory uri) external;
}
