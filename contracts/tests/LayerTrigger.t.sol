pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {LayerTrigger, ILayerTrigger} from "../LayerTrigger.sol";

contract LayerTriggerTest is Test {
    LayerTrigger layerTrigger;

    function setUp() public {
        layerTrigger = new LayerTrigger();
    }

    function testTrigger() public {
        layerTrigger.addTrigger("serviceId", "data1");

        ILayerTrigger.TriggerId triggerId = ILayerTrigger.TriggerId.wrap(1); 
        ILayerTrigger.TriggerResponse memory trigger = layerTrigger.getTrigger(triggerId);

        assertEq(trigger.serviceId, "serviceId");
        assertEq(trigger.creator, address(this));
        assertEq(trigger.data, "data1");
        assertEq(ILayerTrigger.TriggerId.unwrap(trigger.triggerId), ILayerTrigger.TriggerId.unwrap(triggerId));
    }
}