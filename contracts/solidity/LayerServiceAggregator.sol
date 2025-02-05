// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {ILayerServiceHandler} from "@layer/interfaces/ILayerServiceHandler.sol";
import {ILayerServiceAggregator} from "@layer/interfaces/ILayerServiceAggregator.sol";

/**
 * @title LayerServiceAggregator
 * @notice Contract that takes aggregates calls to a ILayerServiceHandler
 */
contract LayerServiceAggregator is ILayerServiceAggregator {
    ILayerServiceHandler private _handler;

    constructor(ILayerServiceHandler handler) {
        _handler = handler;
    }
    // ------------------------------------------------------------------------
    // Custom Errors
    // ------------------------------------------------------------------------
    error InvalidLength();

    function getHandler() external view returns (ILayerServiceHandler) {
        return _handler;
    }
    /**
     * @notice Multi-payload version of the handler's handleSignedData
     * @param datas The arbitrary datas that were signed.
     * @param signatures The signatures of the datas.
     */
    function handleSignedDataMulti(bytes[] calldata datas, bytes[] calldata signatures) external {
        if (datas.length != signatures.length) {
            revert InvalidLength();
        }
        for (uint256 i = 0; i < datas.length; i++) {
            _handler.handleSignedData(datas[i], signatures[i]);
        }
    }
}