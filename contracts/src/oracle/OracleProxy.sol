// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ConfirmedOwner} from "@chainlink/contracts@1.5.0/src/v0.8/shared/access/ConfirmedOwner.sol";

interface IFunctionsConsumer {
    function executeRequest(string[] memory args, uint64 subscriptionId) external returns (bytes32);
}

contract OracleProxy is ConfirmedOwner {
    address public consumer;
    uint64 public subscriptionId;

    struct RequestContext {
        bytes cid;
        address client;
    }

    mapping(bytes32 => RequestContext) public requests;

    event RequestSent(bytes32 indexed requestId, address indexed client, bytes cid);
    event CallbackResult(bytes32 indexed requestId, bool success);

    constructor(address _consumer, uint64 _subId) ConfirmedOwner(msg.sender) {
        consumer = _consumer;
        subscriptionId = _subId;
    }

    function request(bytes memory c_cipher, address callback) external {
        string[] memory args = new string[](1);
        args[0] = string(c_cipher);
        
        bytes32 requestId = IFunctionsConsumer(consumer).executeRequest(args, subscriptionId);
        
        requests[requestId] = RequestContext({
            cid: c_cipher,
            client: callback
        });

        emit RequestSent(requestId, callback, c_cipher);
    }

    /**
     * @notice 由 Consumer 合约回调
     */
    function handleResponse(bytes32 requestId, bytes memory response, bytes memory err) external {
        require(msg.sender == consumer, "Unauthorized: Only Consumer");

        RequestContext memory ctx = requests[requestId];
        if (ctx.client == address(0)) return;

        delete requests[requestId];

        (bool success, ) = ctx.client.call(
            abi.encodeWithSignature("onResponse(bytes,bytes)", ctx.cid, response)
        );

        emit CallbackResult(requestId, success);
    }

    function setConfig(address _consumer, uint64 _subId) external onlyOwner {
        consumer = _consumer;
        subscriptionId = _subId;
    }
}