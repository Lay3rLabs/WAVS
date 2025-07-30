// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

/**
 * @title ISimpleTrigger
 * @author Lay3r Labs
 * @notice Interface for the simple trigger contract
 * @dev This interface defines the functions and events for the simple trigger contract
 */
interface ISimpleTrigger {
    /**
     * @notice TriggerInfo is a struct containing the trigger ID, creator, and data
     * @param triggerId The trigger ID
     * @param creator The creator
     * @param data The data
     */
    struct TriggerInfo {
        TriggerId triggerId;
        address creator;
        bytes data;
    }

    /**
     * @notice TriggerId is a type for the trigger ID
     * @dev This type is used to identify the trigger ID
     * @param triggerId The trigger ID
     */
    type TriggerId is uint64;

    /**
     * @notice Returns the trigger info for a given trigger ID
     * @param triggerId The trigger ID
     * @return triggerInfo The trigger info
     */
    function getTrigger(
        TriggerId triggerId
    ) external view returns (TriggerInfo memory);
}
