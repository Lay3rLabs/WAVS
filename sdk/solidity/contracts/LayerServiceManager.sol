pragma solidity ^0.8.0;

import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";
import "@openzeppelin/contracts/utils/Strings.sol";
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

    function addSignedPayload(
        ILayerServiceManager.SignedPayload calldata signedPayload
    ) view public {
        bytes32 message = keccak256(abi.encode(signedPayload.data));
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
    }

    function addSignedPayloadMulti(
        ILayerServiceManager.SignedPayload[] calldata signedPayloads
    ) view public {
        for (uint32 i = 0; i < signedPayloads.length; i++) {
            LayerServiceManager(address(this)).addSignedPayload(signedPayloads[i]);
        }
    }
}
