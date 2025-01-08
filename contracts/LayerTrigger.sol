pragma solidity ^0.8.0;

import {ILayerTrigger} from "./interfaces/ILayerTrigger.sol";

contract LayerTrigger {
    // Data structures
    struct Trigger {
        address creator;
        bytes data;
    }

    // Storage

    mapping(ILayerTrigger.TriggerId => Trigger) public triggersById;

    mapping(address => ILayerTrigger.TriggerId[]) public triggerIdsByCreator;

    // Events
    event NewTriggerId(ILayerTrigger.TriggerId);
    event WavsTrigger(bytes);

    // Global vars
    ILayerTrigger.TriggerId public nextTriggerId;

    // Functions

    /**
     * @notice Add a new trigger.
     * @param data The request data (bytes).
     */
    function addTrigger(bytes memory data) public {
        // Get the next trigger id
        nextTriggerId = ILayerTrigger.TriggerId.wrap(ILayerTrigger.TriggerId.unwrap(nextTriggerId) + 1);
        ILayerTrigger.TriggerId triggerId = nextTriggerId;

        // Create the trigger
        Trigger memory trigger = Trigger({
            creator: msg.sender,
            data: data
        });

        // update storages
        triggersById[triggerId] = trigger;

        triggerIdsByCreator[msg.sender].push(triggerId);

        // emit the data directly in an event
        emit NewTriggerId(triggerId);
        emit WavsTrigger(data);
    }

    /**
     * @notice Get a single trigger by triggerId.
     * @param triggerId The identifier of the trigger.
     */
    function getTrigger(ILayerTrigger.TriggerId triggerId) public view returns (ILayerTrigger.TriggerResponse memory) {
        Trigger storage trigger = triggersById[triggerId];

        return ILayerTrigger.TriggerResponse({
            triggerId: triggerId,
            creator: trigger.creator,
            data: trigger.data
        });
    }

}
