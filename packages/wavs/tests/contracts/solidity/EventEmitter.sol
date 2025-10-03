// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

contract EventEmitter {
    event IntegerEvent(address indexed from, uint256 value);
    event StringEvent(address indexed from, string value);

    function emitInteger(uint256 value) external {
        emit IntegerEvent(msg.sender, value);
    }

    function emitString(string calldata value) external {
        emit StringEvent(msg.sender, value);
    }
}
