// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {ISimpleTrigger} from "./ISimpleTrigger.sol";

contract SimpleTrigger is ISimpleTrigger {
    // Data structures
    struct Trigger {
        address creator;
        bytes data;
    }

    // Storage

    mapping(TriggerId => Trigger) public triggersById;

    mapping(address => TriggerId[]) public triggerIdsByCreator;

    // Events
    event NewTrigger(bytes);

    // Global vars
    TriggerId public nextTriggerId;

    // Functions

    /**
     * @notice Add a new trigger.
     * @param data The request data (bytes).
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

    /**
     * @notice Get a single trigger by triggerId.
     * @param triggerId The identifier of the trigger.
     */
    function getTrigger(
        TriggerId triggerId
    ) public view returns (TriggerInfo memory) {
        Trigger storage trigger = triggersById[triggerId];

        return TriggerInfo({triggerId: triggerId, creator: trigger.creator, data: trigger.data});
    }
}
