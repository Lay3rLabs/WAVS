// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

contract LogSpam {
    event Spam(uint256 indexed id);

    function emitSpam(uint256 startIndex, uint256 count) external {
        unchecked {
            uint256 end = startIndex + count;
            for (uint256 i = startIndex; i < end; ++i) {
                emit Spam(i);
            }
        }
    }
}
