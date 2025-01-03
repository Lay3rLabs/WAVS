pragma solidity ^0.8.0;

import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";
import "@openzeppelin/contracts/utils/Strings.sol";
import {ILayerTrigger} from "./LayerTrigger.sol";
import {ILayerServiceManager} from "./interfaces/ILayerServiceManager.sol";

contract ReeceLayerServiceManager is ECDSAServiceManagerBase {
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

    /* small optimization, no need to double-save the TriggerId */
    struct SignedData {
        bytes data;
        bytes signature;
    }

    // Storage
    mapping(ILayerTrigger.TriggerId => SignedData) public signedDataByTriggerId;
    uint256 public counter;
    mapping(uint256 => bytes) public autoIncrementStore;
    uint256 public nextStoreId;

    // Events
    event AddedSignedPayloadForTrigger(ILayerTrigger.TriggerId indexed triggerId);
    event CounterIncremented(uint256 newValue);
    event DataStored(uint256 indexed id, bytes data);

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

    function addSignedPayloadForTrigger(
        ILayerServiceManager.SignedPayload calldata signedPayload
    ) public {
        // TODO: My manual ECDSA contract setup is not working as expected. Need to fix this, for now commenting out the signature check
        bytes32 message = keccak256(abi.encode(signedPayload.payload));
        bytes32 ethSignedMessageHash = ECDSAUpgradeable.toEthSignedMessageHash(message);
        bytes4 magicValue = IERC1271Upgradeable.isValidSignature.selector;
        if (
            !(magicValue ==
                ECDSAStakeRegistry(stakeRegistry).isValidSignature(
                    ethSignedMessageHash,
                    signedPayload.signature
                ))
        ) {
            revert InvalidSignature();
        }

        SignedData memory signedData = SignedData({
            data: signedPayload.payload.data,
            signature: signedPayload.signature
        });

        // updating the storage with data responses
        signedDataByTriggerId[signedPayload.payload.triggerId] = signedData;

        // save the data to the autoIncrementStore
        autoIncrementStore[nextStoreId] = signedPayload.payload.data;
        nextStoreId += 1;

        counter += 1;

        // emitting event
        emit AddedSignedPayloadForTrigger(signedPayload.payload.triggerId);
    }

    function addSignedPayloadForTriggerMulti(
        ILayerServiceManager.SignedPayload[] calldata signedPayloads
    ) public {
        for (uint32 i = 0; i < signedPayloads.length; i++) {
            ReeceLayerServiceManager(address(this)).addSignedPayloadForTrigger(signedPayloads[i]);
        }
    }

    // TODO: add onlyOperators modifier
    function incrementCounter() public {
        counter += 1;
        emit CounterIncremented(counter);
    }

    // user calls this,
    function storeData(bytes calldata data) public returns (uint256) {
        uint256 currentId = nextStoreId;
        autoIncrementStore[currentId] = data;
        nextStoreId += 1;

        emit DataStored(currentId, data);
        return currentId;
    }
}
