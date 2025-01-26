// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IServiceHandler} from "@layer-sdk/interfaces/IServiceHandler.sol";
import {ISimpleTrigger} from "./interfaces/ISimpleTrigger.sol";
import {ISimpleSubmit} from "./interfaces/ISimpleSubmit.sol";

contract SimpleSubmit is IServiceHandler {
    address private owner;
    address private serviceManager;

    mapping(ISimpleTrigger.TriggerId => bool) validTriggers;
    mapping(ISimpleTrigger.TriggerId => bytes) datas;
    mapping(ISimpleTrigger.TriggerId => bytes) signatures;

    constructor() {
        owner = msg.sender;
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "Only the owner can call this function.");
        _;
    }

    modifier onlyServiceManager() {
        require(msg.sender == serviceManager, "Only the service manager can call this function.");
        _;
    }

    function setServiceManager(address newServiceManager) external onlyOwner {
        serviceManager = newServiceManager;
    }

    function getServiceManager() public view returns (address) {
        return serviceManager;
    }

    function handleAddPayload(bytes calldata data, bytes calldata signature) external onlyServiceManager {
        ISimpleSubmit.DataWithId memory dataWithId = abi.decode(data, (ISimpleSubmit.DataWithId));

        signatures[dataWithId.triggerId] = signature;
        datas[dataWithId.triggerId] = dataWithId.data;
        validTriggers[dataWithId.triggerId] = true;
    }

    function isValidTriggerId(ISimpleTrigger.TriggerId triggerId) external view returns (bool) {
        return validTriggers[triggerId];
    }

    function getSignature(ISimpleTrigger.TriggerId triggerId) external view returns (bytes memory signature) {
        signature = signatures[triggerId];
    }

    function getData(ISimpleTrigger.TriggerId triggerId) external view returns (bytes memory data) {
        data = datas[triggerId];
    }
}