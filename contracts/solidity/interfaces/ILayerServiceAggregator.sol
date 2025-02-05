// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface ILayerServiceAggregator {
    /**
     * @notice Multi-payload version of handleSignedData
     * @param datas The arbitrary datas that were signed.
     * @param signatures The signatures of the datas.
     */
    function handleSignedDataMulti(bytes[] calldata datas, bytes[] calldata signatures) external;
}