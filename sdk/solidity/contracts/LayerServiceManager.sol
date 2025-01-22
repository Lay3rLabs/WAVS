pragma solidity ^0.8.0;

import {ECDSAServiceManagerBase} from "@eigenlayer/middleware/src/unaudited/ECDSAServiceManagerBase.sol";
import {ECDSAStakeRegistry} from "@eigenlayer/middleware/src/unaudited/ECDSAStakeRegistry.sol";
import {IERC1271Upgradeable} from "@openzeppelin-upgrades/contracts/interfaces/IERC1271Upgradeable.sol";
import {ECDSAUpgradeable} from "@openzeppelin-upgrades/contracts/utils/cryptography/ECDSAUpgradeable.sol";
import "@openzeppelin/contracts/utils/Strings.sol";
import {ILayerServiceManager} from "./interfaces/ILayerServiceManager.sol";

abstract contract LayerServiceManager is ECDSAServiceManagerBase {
    // Modifiers
    modifier onlyOperator() {
        require(
            ECDSAStakeRegistry(stakeRegistry).operatorRegistered(msg.sender),
            "Operator must be the caller"
        );
        _;
    }

    modifier onlyValidPayload(ILayerServiceManager.SignedPayload calldata signedPayload) {
        require(validatePayload(signedPayload), InvalidSignature());
        _;
    }

    modifier onlyValidPayloads(ILayerServiceManager.SignedPayload[] calldata signedPayloads) {
        require(validatePayloadMulti(signedPayloads), InvalidSignature());
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

    // Subcontracts should override this function
    function _handleAddPayload(ILayerServiceManager.SignedPayload calldata signedPayload) internal virtual;

    function addPayload(ILayerServiceManager.SignedPayload calldata signedPayload) public onlyValidPayload(signedPayload) {
        _handleAddPayload(signedPayload);
    }

    function addPayloadMulti(ILayerServiceManager.SignedPayload[] calldata signedPayloads) public onlyValidPayloads(signedPayloads) {
        for (uint32 i = 0; i < signedPayloads.length; i++) {
            _handleAddPayload(signedPayloads[i]);
        }
    }

    function validatePayload(
        ILayerServiceManager.SignedPayload calldata signedPayload
    ) view public returns (bool) {
        bytes32 message = keccak256(signedPayload.data);
        bytes32 ethSignedMessageHash = ECDSAUpgradeable.toEthSignedMessageHash(message);
        bytes4 magicValue = IERC1271Upgradeable.isValidSignature.selector;

        return (magicValue ==
                ECDSAStakeRegistry(stakeRegistry).isValidSignature(
                    ethSignedMessageHash,
                    signedPayload.signature
                )
        );
    }

    function validatePayloadMulti(
        ILayerServiceManager.SignedPayload[] calldata signedPayloads
    ) view public returns (bool) {
        for (uint32 i = 0; i < signedPayloads.length; i++) {
            if(!LayerServiceManager(address(this)).validatePayload(signedPayloads[i])) {
                return false;
            }
        }

        return true;
    }
}
