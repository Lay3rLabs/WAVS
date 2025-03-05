pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {SimpleTrigger, ISimpleTrigger} from "../SimpleTrigger.sol";

contract TriggerTest is Test {
    SimpleTrigger simpleTrigger;

    function setUp() public {
        simpleTrigger = new SimpleTrigger();
    }

    function testTrigger() public {
        simpleTrigger.addTrigger("data1");

        ISimpleTrigger.TriggerId triggerId = ISimpleTrigger.TriggerId.wrap(1); 
        ISimpleTrigger.TriggerInfo memory trigger = simpleTrigger.getTrigger(triggerId);

        assertEq(trigger.creator, address(this));
        assertEq(trigger.data, "data1");
        assertEq(ISimpleTrigger.TriggerId.unwrap(trigger.triggerId), ISimpleTrigger.TriggerId.unwrap(triggerId));
    }
}