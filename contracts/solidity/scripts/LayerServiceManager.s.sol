// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Script.sol";
import {LayerServiceManager} from "../LayerServiceManager.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IDelegationManager} from "@eigenlayer/middleware/lib/eigenlayer-contracts/src/contracts/interfaces/IDelegationManager.sol";
import {Quorum, StrategyParams} from "@eigenlayer/middleware/src/interfaces/IECDSAStakeRegistryEventsAndErrors.sol";
import {IStrategy} from "@eigenlayer/middleware/lib/eigenlayer-contracts/src/contracts/interfaces/IStrategy.sol";

// forge script ./contracts/script/LayerServiceManager.s.sol
contract LayerServiceManagerScript is Script {

    address public delegationManager = vm.envAddress("CLI_EIGEN_CORE_DELEGATION_MANAGER");
    address public rewardsCoordinator = vm.envAddress("CLI_EIGEN_CORE_REWARDS_COORDINATOR");
    address public avsDirectory = vm.envAddress("CLI_EIGEN_CORE_AVS_DIRECTORY");
    address public serviceHandler = vm.envAddress("CLI_EIGEN_SERVICE_HANDLER");

    uint _privateKey = vm.envUint("FOUNDRY_ANVIL_PRIVATE_KEY");

    function setUp() public {}

    function run() public {
        vm.startBroadcast(_privateKey);

        ECDSAStakeRegistry ecdsaRegistry = new ECDSAStakeRegistry(IDelegationManager(delegationManager));

        console.log("delegationManager:", delegationManager);
        console.log("rewardsCoordinator:", rewardsCoordinator);
        console.log("avsDirectory:", avsDirectory);
        console.log("serviceHandler:", serviceHandler);

        LayerServiceManager sm = new LayerServiceManager(
            avsDirectory,
            address(ecdsaRegistry),
            rewardsCoordinator,
            delegationManager,
            serviceHandler
        );


        IStrategy mockStrategy = IStrategy(address(0x1234));
        Quorum memory quorum = Quorum({strategies: new StrategyParams[](1)});
        quorum.strategies[0] = StrategyParams({
            strategy: mockStrategy,
            multiplier: 10_000
        });
        ecdsaRegistry.initialize(address(sm), 0, quorum);

        vm.stopBroadcast();

        console.log("ServiceManager:", address(sm));
        console.log("ecdssa_registry (deployed):", address(ecdsaRegistry));
    }

}