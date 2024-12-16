pragma solidity ^0.8.0;

interface ILayerTrigger {
    struct TriggerResponse {
        TriggerId triggerId;
        string serviceId;
        address creator;
        bytes data;
    }

    type TriggerId is uint64;
}

contract LayerTrigger {
    // Data structures
    struct Trigger {
        string serviceId;
        address creator;
        bytes data;
    }

    // Storage

    mapping(ILayerTrigger.TriggerId => Trigger) public triggersById;

    mapping(string => ILayerTrigger.TriggerId[]) public triggerIdsByServiceId;
    mapping(address => ILayerTrigger.TriggerId[]) public triggerIdsByCreator;

    string[] public allServiceIds;
    mapping(string => bool) public serviceIdExists;

    // Events
    event NewTrigger(string indexed serviceId, ILayerTrigger.TriggerId indexed triggerId);

    // Global vars
    ILayerTrigger.TriggerId public nextTriggerId;

    // Functions

    /**
     * @notice Add a new trigger.
     * @param serviceId The service identifier (string).
     * @param data The request data (bytes).
     */
    function addTrigger(string memory serviceId, bytes memory data) public {
        // Get the next trigger id
        nextTriggerId = ILayerTrigger.TriggerId.wrap(ILayerTrigger.TriggerId.unwrap(nextTriggerId) + 1);
        ILayerTrigger.TriggerId triggerId = nextTriggerId;

        // Create the trigger
        Trigger memory trigger = Trigger({
            serviceId: serviceId,
            creator: msg.sender,
            data: data
        });

        // update storages
        triggersById[triggerId] = trigger;

        triggerIdsByServiceId[serviceId].push(triggerId);

        if (!serviceIdExists[serviceId]) {
            allServiceIds.push(serviceId);
            serviceIdExists[serviceId] = true;
        }

        triggerIdsByCreator[msg.sender].push(triggerId);

        // emit event
        emit NewTrigger(serviceId, triggerId);
    }

    /**
     * @notice Get a single trigger by triggerId.
     * @param triggerId The identifier of the trigger.
     */
    function getTrigger(ILayerTrigger.TriggerId triggerId) public view returns (ILayerTrigger.TriggerResponse memory) {
        Trigger storage trigger = triggersById[triggerId];

        return ILayerTrigger.TriggerResponse({
            triggerId: triggerId,
            serviceId: trigger.serviceId,
            creator: trigger.creator,
            data: trigger.data
        });
    }

    /**
     * @notice Get triggers for a given serviceId, with pagination.
     * @param serviceId The service identifier.
     * @param cursor The starting index (0-based) in the list of triggers for that serviceId.
     * @param limit The maximum number of requests to return.
     *
     * @return triggers An array of TriggerResponse structs.
     * @return nextCursor The next cursor position after the returned set.
     */
    function getTriggersByServiceId(
        string memory serviceId,
        uint256 cursor,
        uint256 limit
    ) external view returns (ILayerTrigger.TriggerResponse[] memory triggers, uint256 nextCursor) {
        uint256 total = triggerIdsByServiceId[serviceId].length;
        if (cursor >= total) {
            // No results if cursor is out of range.
            return (new ILayerTrigger.TriggerResponse[](0), cursor);
        }

        if (limit == 0 || limit > total - cursor) {
            limit = total - cursor;
        }

        triggers = new ILayerTrigger.TriggerResponse[](limit);
        for (uint256 i = 0; i < limit; i++) {
            ILayerTrigger.TriggerId triggerId = triggerIdsByServiceId[serviceId][cursor + i];
            triggers[i] = getTrigger(triggerId);
        }

        nextCursor = cursor + limit;
        return (triggers, nextCursor);
    }
}