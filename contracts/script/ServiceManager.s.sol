// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Script.sol";
import {ReeceLayerServiceManager} from "../ReeceServiceManager.sol";
import {ECDSAStakeRegistry} from "../../lib/eigenlayer-middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IDelegationManager} from "../../lib/eigenlayer-middleware/lib/eigenlayer-contracts/src/contracts/interfaces/IDelegationManager.sol";

// forge script ./contracts/script/ServiceManager.s.sol
contract LayerServiceManagerScript is Script {

    address public delegation_manager = vm.envAddress("CLI_EIGEN_CORE_DELEGATION_MANAGER");
    address public rewards_coordinator = vm.envAddress("CLI_EIGEN_CORE_REWARDS_COORDINATOR");
    address public avs_directory = vm.envAddress("CLI_EIGEN_CORE_AVS_DIRECTORY");

    uint privateKey = vm.envUint("FOUNDRY_ANVIL_PRIVATE_KEY");

    function setUp() public {}

    function run() public {
        vm.startBroadcast(privateKey);

        ECDSAStakeRegistry ecdsa_registry = new ECDSAStakeRegistry(IDelegationManager(delegation_manager));

        console.log("delegation_manager:", delegation_manager);
        console.log("ecdsa_registry (deployed):", address(ecdsa_registry));
        console.log("rewards_coordinator:", rewards_coordinator);
        console.log("avs_directory:", avs_directory);

        ReeceLayerServiceManager sm = new ReeceLayerServiceManager(
            avs_directory,
            address(ecdsa_registry),
            rewards_coordinator,
            delegation_manager
        );

        sm.incrementCounter();
        console.log("counter:", sm.counter());

        vm.stopBroadcast();

        // print out sm address
        console.log("ServiceManager address:", address(sm));
        console.log("ecdssa_registry address:", address(ecdsa_registry));
    }

}
