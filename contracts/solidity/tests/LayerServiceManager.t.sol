// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {LayerServiceManager} from "../LayerServiceManager.sol";
import {ILayerService} from "@layer-sdk/interfaces/ILayerService.sol";

contract LayerServiceManagerTest is Test {
    LayerServiceManager serviceManager;
    LayerServiceHandler serviceHandler;

    function setUp() public {
        serviceHandler = new LayerServiceHandler();
        // TODO - ServiceManager....
    }

    function test_handlesPayload() public {
    }

    function testFail_Subtract43() public {
    }
}


/**
 * @title LayerServiceHandler
 * @notice Example of a contract that knows how to handle validated payloads.
 */
contract LayerServiceHandler is ILayerService {
    /**
     * @notice Called by LayerServiceManager after successful payload signature validation.
     * @dev In a real-world scenario, you could parse `data` or do some state updates here.
     */
    function handleSignedData(bytes calldata data, bytes calldata signature)
        external
    {
        // Additional logic to process `data` would go here
    }
}