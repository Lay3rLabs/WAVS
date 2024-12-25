pragma solidity ^0.8.0;

import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";
import "@openzeppelin/contracts/utils/Strings.sol";
import {ILayerTrigger} from "./interfaces/ILayerTrigger.sol";
import {ILayerServiceManager} from "./interfaces/ILayerServiceManager.sol";
import {IERC721ReceiverUpgradeable} from "@openzeppelin-upgrades/contracts/token/ERC721/IERC721ReceiverUpgradeable.sol";

contract LayerServiceManager is
    ECDSAServiceManagerBase,
    IERC721ReceiverUpgradeable
{
    // Modifiers
    modifier onlyOperator() {
        require(
            ECDSAStakeRegistry(stakeRegistry).operatorRegistered(msg.sender),
            "Operator must be the caller"
        );
        _;
    }

    // Errors

    error InvalidSignature(bytes32 messageHash, bytes signature);
    error TriggerNotFound(ILayerTrigger.TriggerId triggerId);
    error MintingFailed(address creator, string uri);

    // Structs

    /* small optimization, no need to double-save the TriggerId */
    struct SignedData {
        bytes data;
        bytes signature;
    }

    // Storage
    mapping(ILayerTrigger.TriggerId => SignedData) public signedDataByTriggerId;

    // Events
    event AddedSignedPayloadForTrigger(
        ILayerTrigger.TriggerId indexed triggerId
    );

    // Add ILayerTrigger interface instance
    ILayerTrigger public layerTrigger;

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

    // Add initialize function just for layerTrigger
    function initialize(address _layerTrigger) public initializer {
        layerTrigger = ILayerTrigger(_layerTrigger);
    }

    function addSignedPayloadForTrigger(
        ILayerServiceManager.SignedPayload calldata signedPayload
    ) public {
        bytes32 message = keccak256(abi.encode(signedPayload.payload));
        bytes32 ethSignedMessageHash = ECDSAUpgradeable.toEthSignedMessageHash(
            message
        );
        bytes4 magicValue = IERC1271Upgradeable.isValidSignature.selector;

        // Add debug event for signature verification
        // emit DebugBytes32("Checking signature", ethSignedMessageHash);

        if (
            !(magicValue ==
                ECDSAStakeRegistry(stakeRegistry).isValidSignature(
                    ethSignedMessageHash,
                    signedPayload.signature
                ))
        ) {
            revert InvalidSignature(
                ethSignedMessageHash,
                signedPayload.signature
            );
        }

        // Add debug event for trigger lookup - using unwrap() for TriggerId
        // emit DebugBytes32(
        //     "Signature valid, getting trigger",
        //     bytes32(
        //         uint256(
        //             ILayerTrigger.TriggerId.unwrap(
        //                 signedPayload.payload.triggerId
        //             )
        //         )
        //     )
        // );

        // Get the trigger details
        ILayerTrigger.TriggerResponse memory trigger = layerTrigger.getTrigger(
            signedPayload.payload.triggerId
        );

        if (trigger.creator == address(0)) {
            revert TriggerNotFound(signedPayload.payload.triggerId);
        }

        // Add debug event for minting attempt
        emit DebugString("Found trigger, attempting mint", "");

        // Convert bytes to string more safely
        string memory uri = string(
            abi.encodePacked(signedPayload.payload.data)
        );

        try layerTrigger.safeMint(trigger.creator, uri) {
            // Success case
            emit DebugString("Minting successful", "");
        } catch Error(string memory reason) {
            emit DebugString("Minting failed with reason", reason);
            revert MintingFailed(trigger.creator, uri);
        } catch {
            emit DebugString("Minting failed without reason", "");
            revert MintingFailed(trigger.creator, uri);
        }

        SignedData memory signedData = SignedData({
            data: signedPayload.payload.data,
            signature: signedPayload.signature
        });

        // updating the storage with data responses
        signedDataByTriggerId[signedPayload.payload.triggerId] = signedData;

        // emitting event
        emit AddedSignedPayloadForTrigger(signedPayload.payload.triggerId);
    }

    function addSignedPayloadForTriggerMulti(
        ILayerServiceManager.SignedPayload[] calldata signedPayloads
    ) public {
        for (uint32 i = 0; i < signedPayloads.length; i++) {
            LayerServiceManager(address(this)).addSignedPayloadForTrigger(
                signedPayloads[i]
            );
        }
    }

    // Separate debug events to avoid ambiguity
    event DebugBytes32(string message, bytes32 data);
    event DebugString(string message, string data);

    // Add ERC721 receiver function
    function onERC721Received(
        address,
        address,
        uint256,
        bytes memory
    ) public virtual override returns (bytes4) {
        return this.onERC721Received.selector;
    }
}
