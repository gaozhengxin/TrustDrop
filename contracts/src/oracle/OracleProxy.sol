// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Ownable} from "../lib/Ownable.sol";

interface IFunctionsConsumer {
    function executeRequest(
        string[] memory args,
        uint64 subscriptionId
    ) external returns (bytes32);
}

contract OracleProxy is Ownable {
    address public consumer;
    uint64 public subscriptionId;
    address public controller;

    struct RequestContext {
        bytes cid;
        address client;
    }

    mapping(bytes32 => RequestContext) public requests;
    mapping(address => bool) public whiteList;

    event RequestSent(
        bytes32 indexed requestId,
        address indexed client,
        bytes cid
    );
    event CallbackResult(bytes32 indexed requestId, bool success);

    modifier onlyController() {
        require(msg.sender == controller, "Not controller");
        _;
    }

    modifier onlyWhitelisted(address caller) {
        require(whiteList[msg.sender], "Not whitelisted");
        _;
    }

    constructor(address _consumer, uint64 _subId) Ownable(msg.sender) {
        consumer = _consumer;
        subscriptionId = _subId;
    }

    function setController(address newController) public onlyOwner {
        controller = newController;
    }

    function request(
        bytes memory c_cipher,
        address callback
    ) external onlyWhitelisted(msg.sender) {
        string[] memory args = new string[](1);
        args[0] = string(c_cipher);

        bytes32 requestId = IFunctionsConsumer(consumer).executeRequest(
            args,
            subscriptionId
        );

        requests[requestId] = RequestContext({cid: c_cipher, client: callback});

        emit RequestSent(requestId, callback, c_cipher);
    }

    /**
     * @notice 由 Consumer 合约回调
     */
    function handleResponse(
        bytes32 requestId,
        bytes memory response,
        bytes memory err
    ) external {
        require(msg.sender == consumer, "Unauthorized: Only Consumer");

        RequestContext memory ctx = requests[requestId];
        if (ctx.client == address(0)) return;

        // 如果 err 不为空，说明 DON 执行 JS 失败（API 报错、超时等）
        if (err.length > 0) {
            (bool success_err, ) = ctx.client.call(
                abi.encodeWithSignature(
                    "onOracleError(bytes,bytes)",
                    ctx.cid,
                    err
                )
            );
            delete requests[requestId];
            emit CallbackResult(requestId, success_err);
            return;
        }

        delete requests[requestId];

        (bool success, ) = ctx.client.call(
            abi.encodeWithSignature(
                "onResponse(bytes,bytes)",
                ctx.cid,
                response
            )
        );

        emit CallbackResult(requestId, success);
    }

    function setConfig(address _consumer, uint64 _subId) external onlyOwner {
        consumer = _consumer;
        subscriptionId = _subId;
    }

    function setWhitelist(
        address caller,
        bool allowed
    ) external onlyController {
        whiteList[caller] = allowed;
    }
}
