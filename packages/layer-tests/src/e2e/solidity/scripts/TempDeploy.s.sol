// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

import {TempContract} from "../contracts/TempContract.sol";

contract TempDeploy is Script {
    function run(string calldata message) external {
        string memory mnemonic = vm.envString("TEMP_SCRIPT_MNEMONIC");

        (address deployerAddr, ) = deriveRememberKey(mnemonic, 0);

        console.log("Deployer addr: %s", deployerAddr);

        vm.startBroadcast(deployerAddr);

        new TempContract(message);

        vm.stopBroadcast();
    }
}