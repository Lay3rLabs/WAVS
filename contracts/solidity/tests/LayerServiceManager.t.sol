// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {LayerServiceManager} from "../LayerServiceManager.sol";
import {ILayerServiceHandler} from "@layer/interfaces/ILayerServiceHandler.sol";
import {LayerServiceManager} from "@layer/LayerServiceManager.sol";

contract LayerServiceManagerTest is Test {
    LayerServiceManager serviceManager;
    LayerServiceHandler serviceHandler;

    function setUp() public {
        // TODO - ServiceManager....
        serviceHandler = new LayerServiceHandler();
    }

    function test_handlesPayload() public {
    }

    function testFail_Subtract43() public {
    }
}


/**
 * @title LayerServiceHandler
 * @notice Example contract 
 */
contract LayerServiceHandler is ILayerServiceHandler {
    function handleSignedData(bytes calldata data, bytes calldata signature)
        external
    {
    }
    function handleSignedDataMulti(bytes[] calldata datas, bytes[] calldata signatures)
        external
    {
    }
}