// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {ISimpleTrigger} from "./ISimpleTrigger.sol";

/**
 * @title SimpleTrigger
 * @author Lay3r Labs
 * @notice Contract for the simple trigger contract
 * @dev This contract implements the ISimpleTrigger interface
 */
contract SimpleTrigger is ISimpleTrigger {
    /**
     * @notice Trigger is a struct containing the creator and data
     * @param creator The creator
     * @param data The data
     */
    struct Trigger {
        address creator;
        bytes data;
    }

    /// @notice Mapping from trigger ID to trigger
    mapping(TriggerId => Trigger) public triggersById;

    /// @notice Mapping from creator address to trigger IDs
    mapping(address => TriggerId[]) public triggerIdsByCreator;

    /**
     * @notice Event emitted when a new trigger is added
     * @param triggerData The data of the trigger
     */
    event NewTrigger(bytes triggerData);

    /// @notice The next trigger id
    TriggerId public nextTriggerId;

    /**
     * @notice Adds a new trigger
     * @param data The data of the trigger
     */
    function addTrigger(
        bytes memory data
    ) public {
        // Get the next trigger id
        nextTriggerId = TriggerId.wrap(TriggerId.unwrap(nextTriggerId) + 1);
        TriggerId triggerId = nextTriggerId;

        // Create the trigger
        Trigger memory trigger = Trigger({creator: msg.sender, data: data});

        // update storages
        triggersById[triggerId] = trigger;

        triggerIdsByCreator[msg.sender].push(triggerId);

        // emit the id directly in an event

        // now be layer-compatible
        TriggerInfo memory triggerInfo =
            TriggerInfo({triggerId: triggerId, creator: trigger.creator, data: trigger.data});

        emit NewTrigger(abi.encode(triggerInfo));
    }

    /// @inheritdoc ISimpleTrigger
    function getTrigger(
        TriggerId triggerId
    ) public view returns (TriggerInfo memory) {
        Trigger storage trigger = triggersById[triggerId];

        return TriggerInfo({triggerId: triggerId, creator: trigger.creator, data: trigger.data});
    }
}
