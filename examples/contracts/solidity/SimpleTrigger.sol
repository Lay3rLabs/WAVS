pragma solidity ^0.8.0;

import {ILayerTrigger} from "@layer-contracts/interfaces/ILayerTrigger.sol";
import {ISimpleTrigger} from "./interfaces/ISimpleTrigger.sol";

contract SimpleTrigger {
    // Data structures
    struct Trigger {
        address creator;
        bytes data;
    }

    // Storage

    mapping(ISimpleTrigger.TriggerId => Trigger) public triggersById;

    mapping(address => ISimpleTrigger.TriggerId[]) public triggerIdsByCreator;

    // Events
    event NewTriggerId(ISimpleTrigger.TriggerId);

    // Global vars
    ISimpleTrigger.TriggerId public nextTriggerId;

    // Functions

    /**
     * @notice Add a new trigger.
     * @param data The request data (bytes).
     */
    function addTrigger(bytes memory data) public {
        // Get the next trigger id
        nextTriggerId = ISimpleTrigger.TriggerId.wrap(ISimpleTrigger.TriggerId.unwrap(nextTriggerId) + 1);
        ISimpleTrigger.TriggerId triggerId = nextTriggerId;

        // Create the trigger
        Trigger memory trigger = Trigger({
            creator: msg.sender,
            data: data
        });

        // update storages
        triggersById[triggerId] = trigger;

        triggerIdsByCreator[msg.sender].push(triggerId);

        // emit the id directly in an event
        emit NewTriggerId(triggerId);

        // now be layer-compatible
        ISimpleTrigger.TriggerInfo memory triggerInfo = ISimpleTrigger.TriggerInfo({
            triggerId: triggerId,
            creator: trigger.creator,
            data: trigger.data
        });

        emit ILayerTrigger.LayerTriggerEvent(abi.encode(triggerInfo));
    }

    /**
     * @notice Get a single trigger by triggerId.
     * @param triggerId The identifier of the trigger.
     */
    function getTrigger(ISimpleTrigger.TriggerId triggerId) public view returns (ISimpleTrigger.TriggerInfo memory) {
        Trigger storage trigger = triggersById[triggerId];

        return ISimpleTrigger.TriggerInfo({
            triggerId: triggerId,
            creator: trigger.creator,
            data: trigger.data
        });
    }

}
