// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract OracleClient {
    address public proxy;
    
    // CID => 可访问的结束时间戳
    mapping(bytes => uint256) public availableEndTime;
    // CID => 状态 (0: Ensured, 1: Retrieveable, 2: Not Retrieveable)
    mapping(bytes => uint256) public cidStatus;

    event FileStatusUpdated(bytes cid, uint256 status, uint256 endTime);

    constructor(address _proxy) {
        proxy = _proxy;
    }

    function onResponse(bytes memory cCipher, bytes memory response) external {
        require(msg.sender == proxy, "Only proxy can callback");

        if (response.length >= 64) {
            (uint256 status, uint256 endTime) = abi.decode(response, (uint256, uint256));
            assert(endTime < block.timestamp + 1000 days);
            
            cidStatus[cCipher] = status;
            // 根据逻辑：如果不是 Ensured (0)，endTime 应该是 0
            availableEndTime[cCipher] = endTime;

            emit FileStatusUpdated(cCipher, status, endTime);
        }
    }
}