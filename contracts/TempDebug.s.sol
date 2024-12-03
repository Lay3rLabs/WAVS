// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.12;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

import {CoreDeploymentLib} from "./utils/CoreDeploymentLib.sol";
import {UpgradeableProxyLib} from "./utils/UpgradeableProxyLib.sol";
import {TransparentUpgradeableProxy} from
    "@openzeppelin/contracts/proxy/transparent/TransparentUpgradeableProxy.sol";
import {DelegationManager} from "@eigenlayer/contracts/core/DelegationManager.sol";
import {IStrategyManager} from "@eigenlayer/contracts/interfaces/IStrategyManager.sol";
import {ISlasher} from "@eigenlayer/contracts/interfaces/ISlasher.sol";
import {IEigenPodManager} from "@eigenlayer/contracts/interfaces/IEigenPodManager.sol";
import {ProxyAdmin} from "@openzeppelin/contracts/proxy/transparent/ProxyAdmin.sol";

struct DeployResult {
    address delegationManager;
}

contract DeployEigenlayerCore is Script {
    using CoreDeploymentLib for *;
    using UpgradeableProxyLib for address;

    address internal deployer;
    address internal proxyAdmin;
    DeployResult internal deploymentResult;
    CoreDeploymentLib.DeploymentConfigData internal configData;

    function setUp() public virtual {
        deployer = vm.rememberKey(vm.envUint("PRIVATE_KEY"));
        vm.label(deployer, "Deployer");
    }

    function run() external {
        vm.startBroadcast(deployer);
        proxyAdmin = UpgradeableProxyLib.deployProxyAdmin();
        deploymentResult = deployContracts(
            proxyAdmin,
            configData
        );
        vm.stopBroadcast();
    }

    function deployContracts(
        address proxyAdmin,
        CoreDeploymentLib.DeploymentConfigData memory configData
    ) internal returns (DeployResult memory) {
        DeployResult memory deploy_result;
        CoreDeploymentLib.DeploymentData memory proxies;

        proxies.delegationManager = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);
        proxies.avsDirectory = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);
        proxies.strategyManager = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);
        proxies.eigenPodManager = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);
        proxies.rewardsCoordinator = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);
        proxies.eigenPodBeacon = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);
        proxies.pauserRegistry = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);
        proxies.strategyFactory = UpgradeableProxyLib.setUpEmptyProxy(proxyAdmin);

        // Deploy the implementation contracts, using the proxy contracts as inputs
        address delegationManagerImpl = address(
            new DelegationManager(
                IStrategyManager(proxies.strategyManager),
                ISlasher(address(0)),
                IEigenPodManager(proxies.eigenPodManager)
            )
        );

        TransparentUpgradeableProxy proxy = TransparentUpgradeableProxy(payable(proxies.delegationManager)); 
        address proxy_impl_addr_before = ProxyAdmin(proxyAdmin).getProxyImplementation(proxy);
        console.log("proxy implementation before upgrade:", proxy_impl_addr_before);

        ProxyAdmin(proxyAdmin).upgrade(TransparentUpgradeableProxy(payable(proxies.delegationManager)), delegationManagerImpl);

        address proxy_impl_addr_after = ProxyAdmin(proxyAdmin).getProxyImplementation(proxy);
        console.log("proxy implementation after upgrade:", proxy_impl_addr_after);

        deploy_result.delegationManager = delegationManagerImpl;

        return deploy_result;
    }
}