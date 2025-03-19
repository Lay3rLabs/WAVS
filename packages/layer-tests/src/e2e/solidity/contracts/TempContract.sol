// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TempContract {
    string public _message;

    constructor(string memory message) {
        _message = message;
    }

    function getMessage() public view returns (string memory) {
        return _message;
    }
}