pragma solidity ^0.8.0;

import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";
import "@openzeppelin/contracts/utils/Strings.sol";
import {ILayerTrigger} from "./LayerTrigger.sol";
import {ILayerServiceManager} from "./interfaces/ILayerServiceManager.sol";

contract LayerServiceManager is ECDSAServiceManagerBase {
    // Modifiers
    modifier onlyOperator() {
        require(
            ECDSAStakeRegistry(stakeRegistry).operatorRegistered(msg.sender),
            "Operator must be the caller"
        );
        _;
    }

    // Errors

    error InvalidSignature();

    // Structs


    // Storage
    mapping(ILayerTrigger.TriggerId => ILayerServiceManager.SignedPayload) public signedPayloadByTriggerId;

    // Events
    event AddedSignedPayloadForTrigger(ILayerTrigger.TriggerId indexed triggerId);

    // Functions
    constructor(
        address _avsDirectory,
        address _stakeRegistry,
        address _rewardsCoordinator,
        address _delegationManager
    )
        ECDSAServiceManagerBase(
            _avsDirectory,
            _stakeRegistry,
            _rewardsCoordinator,
            _delegationManager
        )
    {}

    function addRawData(
        bytes calldata data, 
        bytes calldata signature
    ) view public {
        bytes32 message = keccak256(abi.encode(data));
        bytes32 ethSignedMessageHash = ECDSAUpgradeable.toEthSignedMessageHash(message);
        bytes4 magicValue = IERC1271Upgradeable.isValidSignature.selector;

        if (
            !(magicValue ==
                ECDSAStakeRegistry(stakeRegistry).isValidSignature(
                    ethSignedMessageHash,
                    signature
                ))
        ) {
            revert InvalidSignature();
        }
    }

    // TODO - contracts should basically do this, but derive the extra stuff from signedPayload.data
    function addSignedPayloadForTrigger(
        ILayerServiceManager.SignedPayload calldata signedPayload,
        ILayerTrigger.TriggerId triggerId
    ) public {
        addRawData(signedPayload.data, signedPayload.signature);

        // updating the storage with data responses
        signedPayloadByTriggerId[triggerId] = signedPayload;

        // emitting event
        emit AddedSignedPayloadForTrigger(triggerId);
    }

    function addSignedPayloadMulti(
        ILayerServiceManager.SignedPayload[] calldata signedPayloads
    ) view public {
        for (uint32 i = 0; i < signedPayloads.length; i++) {
            LayerServiceManager(address(this)).addRawData(signedPayloads[i].data, signedPayloads[i].signature);
        }
    }
}
