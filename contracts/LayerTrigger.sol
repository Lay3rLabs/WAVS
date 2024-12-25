// SPDX-License-Identifier: MIT
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.22;

import {AccessControlUpgradeable} from "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import {EIP712Upgradeable} from "@openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol";
import {ERC721Upgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC721/ERC721Upgradeable.sol";
import {ERC721BurnableUpgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC721/extensions/ERC721BurnableUpgradeable.sol";
import {ERC721EnumerableUpgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC721/extensions/ERC721EnumerableUpgradeable.sol";
import {ERC721URIStorageUpgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC721/extensions/ERC721URIStorageUpgradeable.sol";
import {ERC721VotesUpgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC721/extensions/ERC721VotesUpgradeable.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {ILayerTrigger} from "./interfaces/ILayerTrigger.sol";

/// LOL it has to be called this for the current tooling to work... that should be fixed.
/// @custom:security-contact meow@daodao.zone
contract LayerTrigger is
    Initializable,
    ERC721Upgradeable,
    ERC721EnumerableUpgradeable,
    ERC721URIStorageUpgradeable,
    ERC721BurnableUpgradeable,
    AccessControlUpgradeable,
    EIP712Upgradeable,
    ERC721VotesUpgradeable
{
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    uint256 private _nextTokenId;

    // Layer Stuff
    struct Trigger {
        string serviceId;
        string workflowId;
        address creator;
        bytes data;
    }

    mapping(ILayerTrigger.TriggerId => Trigger) public triggersById;
    mapping(address => ILayerTrigger.TriggerId[]) public triggerIdsByCreator;

    event NewTrigger(
        string serviceId,
        string workflowId,
        ILayerTrigger.TriggerId indexed triggerId
    );

    ILayerTrigger.TriggerId public nextTriggerId;

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    // TODO minter should be the service contract.
    function initialize(
        address defaultAdmin,
        address minter
    ) public initializer {
        __ERC721_init("Collective Super Intelligence", "CSI");
        __ERC721Enumerable_init();
        __ERC721URIStorage_init();
        __ERC721Burnable_init();
        __AccessControl_init();
        __EIP712_init("Collective Super Intelligence", "1");
        __ERC721Votes_init();

        _grantRole(DEFAULT_ADMIN_ROLE, defaultAdmin);
        _grantRole(MINTER_ROLE, minter);
    }

    // TODO mod to take payment? Maybe make a proper minter contract?
    /**
     * @notice Add a new trigger.
     * @param serviceId The service identifier (string).
     * @param data The request data (bytes).
     */
    function addTrigger(
        string memory serviceId,
        string memory workflowId,
        bytes memory data
    ) public {
        // Get the next trigger id
        nextTriggerId = ILayerTrigger.TriggerId.wrap(
            ILayerTrigger.TriggerId.unwrap(nextTriggerId) + 1
        );
        ILayerTrigger.TriggerId triggerId = nextTriggerId;

        // Create the trigger
        Trigger memory trigger = Trigger({
            serviceId: serviceId,
            workflowId: workflowId,
            creator: msg.sender,
            data: data
        });

        // update storages
        triggersById[triggerId] = trigger;

        triggerIdsByCreator[msg.sender].push(triggerId);

        // TODO why don't we emit data?
        // emit event
        emit NewTrigger(serviceId, workflowId, triggerId);
    }

    /**
     * @notice Get a single trigger by triggerId.
     * @param triggerId The identifier of the trigger.
     */
    function getTrigger(
        ILayerTrigger.TriggerId triggerId
    ) public view returns (ILayerTrigger.TriggerResponse memory) {
        Trigger storage trigger = triggersById[triggerId];

        return
            ILayerTrigger.TriggerResponse({
                triggerId: triggerId,
                workflowId: trigger.workflowId,
                serviceId: trigger.serviceId,
                creator: trigger.creator,
                data: trigger.data
            });
    }

    function safeMint(
        address to,
        string memory uri
    ) public onlyRole(MINTER_ROLE) {
        uint256 tokenId = _nextTokenId++;
        _safeMint(to, tokenId);
        _setTokenURI(tokenId, uri);
    }

    // The following functions are overrides required by Solidity.

    function _update(
        address to,
        uint256 tokenId,
        address auth
    )
        internal
        override(
            ERC721Upgradeable,
            ERC721EnumerableUpgradeable,
            ERC721VotesUpgradeable
        )
        returns (address)
    {
        return super._update(to, tokenId, auth);
    }

    function _increaseBalance(
        address account,
        uint128 value
    )
        internal
        override(
            ERC721Upgradeable,
            ERC721EnumerableUpgradeable,
            ERC721VotesUpgradeable
        )
    {
        super._increaseBalance(account, value);
    }

    function tokenURI(
        uint256 tokenId
    )
        public
        view
        override(ERC721Upgradeable, ERC721URIStorageUpgradeable)
        returns (string memory)
    {
        return super.tokenURI(tokenId);
    }

    function supportsInterface(
        bytes4 interfaceId
    )
        public
        view
        override(
            ERC721Upgradeable,
            ERC721EnumerableUpgradeable,
            ERC721URIStorageUpgradeable,
            AccessControlUpgradeable
        )
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
}
