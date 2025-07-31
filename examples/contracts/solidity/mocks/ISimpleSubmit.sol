// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import {ISimpleTrigger} from "./ISimpleTrigger.sol";
import {IWavsServiceHandler} from "../interfaces/IWavsServiceHandler.sol";

/**
 * @title ISimpleSubmit
 * @author Lay3r Labs
 * @notice Interface for the simple submit contract
 * @dev This interface defines the functions and events for the simple submit contract
 */
interface ISimpleSubmit {
    /**
     * @notice DataWithId is a struct containing a trigger ID and data
     * @param triggerId The trigger ID
     * @param data The data
     */
    struct DataWithId {
        ISimpleTrigger.TriggerId triggerId;
        bytes data;
    }

    /**
     * @notice SignedData is a struct containing the data and signature data
     * @param data The data
     * @param signatureData The signature data
     * @param envelope The envelope
     */
    struct SignedData {
        bytes data;
        IWavsServiceHandler.SignatureData signatureData;
        IWavsServiceHandler.Envelope envelope;
    }

    /**
     * @notice Returns the signed data for a given trigger ID
     * @param triggerId The trigger ID
     * @return signedData The signed data
     */
    function getSignedData(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (SignedData memory);

    /**
     * @notice Returns the data with ID for a given trigger ID
     * @param triggerId The trigger ID
     * @return dataWithId The data with ID
     */
    function getDataWithId(
        ISimpleTrigger.TriggerId triggerId
    ) external view returns (DataWithId memory);
}
