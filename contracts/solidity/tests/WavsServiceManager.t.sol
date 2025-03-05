// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {WavsServiceManager} from "../WavsServiceManager.sol";
import {IWavsServiceHandler} from "@layer/interfaces/IWavsServiceHandler.sol";
import {WavsServiceManager} from "@layer/WavsServiceManager.sol";

contract WavsServiceManagerTest is Test {
    WavsServiceManager serviceManager;
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
contract LayerServiceHandler is IWavsServiceHandler {
    function handleSignedData(bytes calldata data, bytes calldata signature)
        external
    {
    }
    function handleSignedDataMulti(bytes[] calldata datas, bytes[] calldata signatures)
        external
    {
    }
}
